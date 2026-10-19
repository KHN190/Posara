#![cfg(all(feature = "fs", feature = "compiler"))]

use std::path::{Path, PathBuf};

use posara::fs::resolve;

#[test]
fn resolve_accepts_relative_paths_and_rejects_escapes() {
    let root = Path::new("/root");
    for (p, want) in [
        ("a/b.spr", Some(PathBuf::from("/root/a/b.spr"))),
        ("./a", Some(PathBuf::from("/root/a"))),
        ("..", None),
        ("../etc/passwd", None),
        ("a/../../b", None),
        ("/etc/passwd", None),
    ] {
        assert_eq!(resolve(root, p), want, "{p}");
    }
}

// fs round-trip: a cart writes bytes then reads them back, proving
// fs_open/fs_writes/fs_close/fs_reads survive the native rename + wire together.
#[test]
fn fs_write_read_roundtrip() {
    use std::io::Write;
    use posara::Host;
    use posara::runner::{compile_abe, Stepper};

    let root = std::env::temp_dir().join("posara_fs_rt");
    std::fs::create_dir_all(&root).unwrap();
    let cart = root.join("rt.abe");
    // write "A" (0x41) to a file, read it back, paint it across the screen.
    let src = "@cart\nfn main() -> <frame, Graphics, IO> Unit {\n  \
        let w = fs_open(\"out.bin\", 22);\n  \
        let _ = fs_writes(w, \"A\");\n  \
        let _ = fs_close(w);\n  \
        gfx_screen(4, 4);\n  \
        loop {\n    \
        let r = fs_open(\"out.bin\", 1);\n    \
        let s = fs_reads(r, 1);\n    \
        let _ = fs_close(r);\n    \
        gfx_cls(s.byte_at(0));\n    \
        gfx_commit();\n    frame.present()\n  }\n}\n";
    std::fs::File::create(&cart).unwrap().write_all(src.as_bytes()).unwrap();

    let host = Host::new_with(root.clone(), true, true).unwrap();
    let r = compile_abe(&cart, &host).unwrap();
    let mut step = Stepper::start_named(r.module, r.static_names, r.fn_names, &host).unwrap();
    step.frame().unwrap();

    assert!(host.gfx.fb.borrow().buf.iter().all(|&c| c == 0x41), "round-trip byte mismatch");
    std::fs::remove_dir_all(&root).ok();
}

mod posix_model {
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    use std::rc::Rc;

    use posara::fs::Storage;
    use proptest::prelude::*;

    type Inode = Rc<RefCell<Vec<u8>>>;

    struct Fd { file: Inode, pos: usize, read: bool, write: bool, append: bool }

    #[derive(Default)]
    pub struct Model { names: BTreeMap<String, Inode>, fds: Vec<Option<Fd>> }

    impl Model {
        fn open(&mut self, path: &str, flags: i64) -> i64 {
            let (read, append) = (flags & 1 != 0, flags & 8 != 0);
            let write = flags & 2 != 0 || append;
            let (create, trunc) = (flags & 4 != 0, flags & 16 != 0 && !append);
            if !read && !write { return -1; }
            if !write && (create || trunc) { return -1; }
            let file = match self.names.get(path) {
                Some(f) => Rc::clone(f),
                None if create => Rc::clone(self.names.entry(path.into()).or_default()),
                None => return -1,
            };
            if trunc { file.borrow_mut().clear(); }
            let fd = Fd { file, pos: 0, read, write, append };
            match self.fds.iter().position(Option::is_none) {
                Some(i) => { self.fds[i] = Some(fd); i as i64 }
                None => { self.fds.push(Some(fd)); self.fds.len() as i64 - 1 }
            }
        }
        fn fd(&mut self, fd: i64) -> Option<&mut Fd> { usize::try_from(fd).ok().and_then(|i| self.fds.get_mut(i)?.as_mut()) }
        fn close(&mut self, fd: i64) -> bool {
            usize::try_from(fd).ok().and_then(|i| self.fds.get_mut(i)).is_some_and(|s| s.take().is_some())
        }
        fn seek(&mut self, fd: i64, whence: i64, off: i64) -> i64 {
            let Some(f) = self.fd(fd) else { return -1; };
            let len = f.file.borrow().len() as i64;
            let to = match whence { 0 => off.max(0), 1 => f.pos as i64 + off, 2 => len + off, _ => return -1 };
            if to < 0 { return -1; }
            f.pos = to as usize;
            to
        }
        fn read(&mut self, fd: i64, n: usize) -> Option<Vec<u8>> {
            let f = self.fd(fd)?;
            if !f.read { return Some(vec![]); }
            let data = f.file.borrow();
            let start = f.pos.min(data.len());
            let end = (start + n).min(data.len());
            let out = data[start..end].to_vec();
            drop(data);
            f.pos = end.max(f.pos);
            Some(out)
        }
        fn write(&mut self, fd: i64, bytes: &[u8]) -> Option<usize> {
            let f = self.fd(fd)?;
            if !f.write { return None; }
            if bytes.is_empty() { return Some(0); }
            let mut data = f.file.borrow_mut();
            if f.append { f.pos = data.len(); }
            if data.len() < f.pos { data.resize(f.pos, 0); }
            let end = f.pos + bytes.len();
            if data.len() < end { data.resize(end, 0); }
            data[f.pos..end].copy_from_slice(bytes);
            f.pos = end;
            Some(bytes.len())
        }
        fn exists(&self, path: &str) -> bool { self.names.contains_key(path) }
        fn remove(&mut self, path: &str) -> bool { self.names.remove(path).is_some() }
        fn list(&self) -> Vec<String> { self.names.keys().cloned().collect() }
    }

    #[derive(Debug, Clone)]
    pub enum Op { Open(usize, i64), Close(i64), Seek(i64, i64, i64), Read(i64, usize), Write(i64, Vec<u8>), Exists(usize), Remove(usize) }

    const NAMES: [&str; 3] = ["a.bin", "b.txt", "c"];

    pub fn op() -> impl Strategy<Value = Op> {
        let fd = -1i64..5;
        prop_oneof![
            3 => (0..3usize, 0i64..32).prop_map(|(p, f)| Op::Open(p, f)),
            1 => fd.clone().prop_map(Op::Close),
            2 => (fd.clone(), -1i64..4, -12i64..24).prop_map(|(d, w, o)| Op::Seek(d, w, o)),
            3 => (fd.clone(), 0usize..16).prop_map(|(d, n)| Op::Read(d, n)),
            3 => (fd.clone(), proptest::collection::vec(any::<u8>(), 0..10)).prop_map(|(d, b)| Op::Write(d, b)),
            1 => (0..3usize).prop_map(Op::Exists),
            1 => (0..3usize).prop_map(Op::Remove),
        ]
    }

    pub fn replay(s: &dyn Storage, ops: &[Op]) -> Result<(), String> {
        let mut m = Model::default();
        for (i, o) in ops.iter().enumerate() {
            let (got, want) = match o {
                Op::Open(p, f) => (s.open(NAMES[*p], *f).to_string(), m.open(NAMES[*p], *f).to_string()),
                Op::Close(d) => (s.close(*d).to_string(), m.close(*d).to_string()),
                Op::Seek(d, w, off) => (s.seek(*d, *w, *off).to_string(), m.seek(*d, *w, *off).to_string()),
                Op::Read(d, n) => (format!("{:?}", s.read(*d, *n)), format!("{:?}", m.read(*d, *n))),
                Op::Write(d, b) => (format!("{:?}", s.write(*d, b)), format!("{:?}", m.write(*d, b))),
                Op::Exists(p) => (s.exists(NAMES[*p]).to_string(), m.exists(NAMES[*p]).to_string()),
                Op::Remove(p) => (s.remove(NAMES[*p]).to_string(), m.remove(NAMES[*p]).to_string()),
            };
            if got != want { return Err(format!("op {i} {o:?}: got {got}, want {want}")); }
            let names: Vec<String> = s.list("").into_iter().filter(|n| NAMES.contains(&n.as_str())).collect();
            if names != m.list() { return Err(format!("op {i} {o:?}: list {names:?}, want {:?}", m.list())); }
        }
        Ok(())
    }

    #[cfg(feature = "storage-disk")]
    fn scratch_dir() -> std::path::PathBuf {
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let d = std::env::temp_dir().join(format!("posara_fs_model_{}_{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    proptest! {
        #[test]
        fn memory_files_behave_like_posix_files(ops in proptest::collection::vec(op(), 0..40)) {
            let r = replay(&posara::fs::MemStorage::new(), &ops);
            prop_assert!(r.is_ok(), "{}", r.unwrap_err());
        }

        #[cfg(feature = "storage-disk")]
        #[test]
        fn disk_files_behave_like_posix_files(ops in proptest::collection::vec(op(), 0..40)) {
            let dir = scratch_dir();
            let r = replay(&posara::backend::DiskStorage::new(dir.clone()), &ops);
            let _ = std::fs::remove_dir_all(&dir);
            prop_assert!(r.is_ok(), "{}", r.unwrap_err());
        }

        #[test]
        fn uploads_stay_within_the_quotas(files in proptest::collection::vec((0usize..24, 0usize..(1 << 21)), 0..30)) {
            use posara::fs::storage::{MAX_FILES, MAX_FILE_BYTES, MAX_TOTAL_BYTES};
            let s = posara::fs::MemStorage::new();
            let mut model: BTreeMap<String, usize> = BTreeMap::new();
            for (k, len) in files {
                let name = format!("f{k}");
                let others: usize = model.iter().filter(|(n, _)| **n != name).map(|(_, l)| *l).sum();
                let ok = len <= MAX_FILE_BYTES && (model.contains_key(&name) || model.len() < MAX_FILES) && others + len <= MAX_TOTAL_BYTES;
                prop_assert_eq!(s.add_file(&name, vec![7; len]).is_ok(), ok);
                if ok { model.insert(name, len); }
            }
            prop_assert_eq!(s.file_names(), model.keys().cloned().collect::<Vec<_>>());
        }
    }
}

#[cfg(feature = "gfx")]
mod natives {
    use proptest::prelude::*;

    fn run(src: &str) -> Result<Vec<u16>, String> {
        use posara::runner::{compile_source, Stepper};
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!("posara_fs_native_{}_{n}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let host = posara::Host::new_with(root.clone(), true, true).unwrap();
        let r = compile_source(src, &host).unwrap_or_else(|e| panic!("{e}\n{src}"));
        let out = Stepper::start_named(r.module, r.static_names, r.fn_names, &host).map(|_| host.gfx.fb.borrow().buf.clone());
        std::fs::remove_dir_all(&root).ok();
        out
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: ProptestConfig::default().cases.min(48), ..ProptestConfig::default() })]

        #[test]
        fn fs_read_returns_what_fs_write_put_there_zero_padded(off in 0usize..12, data in proptest::collection::vec(any::<u8>(), 1..16), n in 1usize..40) {
            let list = data.iter().map(|b| b.to_string()).collect::<Vec<_>>().join(", ");
            let src = format!("@cart\nfn main() -> <frame, Graphics, IO> Unit {{\n  \
                let w = fs_open(\"p.bin\", 22);\n  let _ = fs_seek(w, {off}, 0);\n  let _ = fs_write(w, [{list}]);\n  let _ = fs_close(w);\n  \
                let r = fs_open(\"p.bin\", 1);\n  let d = fs_read(r, {n});\n  let _ = fs_close(r);\n  \
                gfx_screen({n}, 1);\n  for i in 0..{n} {{ gfx_pset(i, 0, d.byte_at(i)) }};\n  gfx_commit();\n  loop {{ frame.present() }}\n}}\n");
            let mut file = vec![0u8; off];
            file.extend(&data);
            let want: Vec<u16> = (0..n).map(|i| file.get(i).copied().unwrap_or(0) as u16).collect();
            prop_assert_eq!(run(&src).unwrap(), want);
        }
    }

    #[test]
    fn an_absurd_read_size_is_a_runtime_error_not_an_abort() {
        let src = "@cart\nfn main() -> <frame, Graphics, IO> Unit {\n  let w = fs_open(\"p.bin\", 22);\n  let d = fs_read(w, 1099511627776);\n  gfx_screen(1, 1);\n  gfx_pset(0, 0, d.byte_at(0));\n  loop { frame.present() }\n}\n";
        assert!(run(src).unwrap_err().contains("fs_read"));
    }
}
