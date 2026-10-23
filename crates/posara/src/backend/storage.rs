// Storage seam. Portable fs natives call Storage; std::fs lives only in
// DiskStorage (desktop). Web gets an in-memory VFS.
use std::path::{Component, Path, PathBuf};

pub use posara_core::fs::storage::*;

// Join rel under root, rejecting absolute paths and `..` escape. Lexical only.
pub fn resolve(root: &Path, rel: &str) -> Option<PathBuf> {
    let mut out = root.to_path_buf();
    for comp in Path::new(rel).components() {
        match comp {
            Component::Normal(c) => out.push(c),
            Component::CurDir => {}
            _ => return None,
        }
    }
    Some(out)
}

// -------- desktop: real filesystem --------
#[cfg(feature = "storage-disk")]
pub use disk::DiskStorage;

#[cfg(feature = "storage-disk")]
mod disk {
    use super::*;
    use std::cell::RefCell;
    use std::fs::{File, OpenOptions};
    use std::io::{Read, Seek, SeekFrom, Write};
    use std::time::UNIX_EPOCH;

    #[derive(Default)]
    struct FdTable {
        files: Vec<Option<File>>,
    }
    impl FdTable {
        fn insert(&mut self, f: File) -> i64 {
            if let Some(i) = self.files.iter().position(|s| s.is_none()) {
                self.files[i] = Some(f);
                i as i64
            } else {
                self.files.push(Some(f));
                (self.files.len() - 1) as i64
            }
        }
        fn get(&mut self, fd: i64) -> Option<&mut File> {
            usize::try_from(fd).ok().and_then(|i| self.files.get_mut(i)).and_then(|s| s.as_mut())
        }
        fn close(&mut self, fd: i64) -> bool {
            match usize::try_from(fd).ok().and_then(|i| self.files.get_mut(i)) {
                Some(slot @ Some(_)) => { *slot = None; true }
                _ => false,
            }
        }
    }

    pub struct DiskStorage {
        root: PathBuf,
        fds: RefCell<FdTable>,
    }
    impl DiskStorage {
        pub fn new(root: PathBuf) -> Self { Self { root, fds: RefCell::new(FdTable::default()) } }
        fn path(&self, rel: &str) -> Option<PathBuf> { resolve(&self.root, rel) }
    }

    impl Storage for DiskStorage {
        fn open(&self, rel: &str, flags: i64) -> i64 {
            let Some(path) = self.path(rel) else { return -1; };
            let mut o = OpenOptions::new();
            o.read(flags & F_READ != 0)
                .write(flags & (F_WRITE | F_APPEND) != 0)
                .create(flags & F_CREATE != 0)
                .append(flags & F_APPEND != 0)
                .truncate(flags & F_TRUNC != 0 && flags & F_APPEND == 0);
            match o.open(&path) {
                Ok(f) => self.fds.borrow_mut().insert(f),
                Err(e) => { eprintln!("• fs_open: {} ({e})", path.display()); -1 }
            }
        }
        fn close(&self, fd: i64) -> bool { self.fds.borrow_mut().close(fd) }
        fn seek(&self, fd: i64, whence: i64, off: i64) -> i64 {
            let mut t = self.fds.borrow_mut();
            let Some(f) = t.get(fd) else { return -1; };
            let from = match whence {
                0 => SeekFrom::Start(off.max(0) as u64),
                1 => SeekFrom::Current(off),
                2 => SeekFrom::End(off),
                _ => return -1,
            };
            f.seek(from).map(|p| p as i64).unwrap_or(-1)
        }
        fn read(&self, fd: i64, n: usize) -> Option<Vec<u8>> {
            let mut t = self.fds.borrow_mut();
            let f = t.get(fd)?;
            if n == 0 { return Some(Vec::new()); }
            let mut buf = vec![0u8; n];
            let got = f.read(&mut buf).ok()?;
            buf.truncate(got);
            Some(buf)
        }
        fn write(&self, fd: i64, data: &[u8]) -> Option<usize> {
            let mut t = self.fds.borrow_mut();
            let f = t.get(fd)?;
            f.write(data).ok()
        }
        fn exists(&self, rel: &str) -> bool { self.path(rel).map(|p| p.exists()).unwrap_or(false) }
        fn mkdir(&self, rel: &str) -> bool { self.path(rel).map(|p| std::fs::create_dir_all(p).is_ok()).unwrap_or(false) }
        fn remove(&self, rel: &str) -> bool {
            self.path(rel).map(|p| {
                if p.is_dir() { std::fs::remove_dir_all(p).is_ok() } else { std::fs::remove_file(p).is_ok() }
            }).unwrap_or(false)
        }
        fn list(&self, rel: &str) -> Vec<String> {
            self.path(rel).and_then(|p| std::fs::read_dir(p).ok()).map(|rd| {
                let mut names: Vec<String> = rd.filter_map(|e| e.ok())
                    .filter_map(|e| e.file_name().into_string().ok()).collect();
                names.sort();
                names
            }).unwrap_or_default()
        }
        fn mtime(&self, rel: &str) -> Option<u64> {
            let p = self.path(rel)?;
            let m = std::fs::metadata(&p).ok()?.modified().ok()?;
            Some(m.duration_since(UNIX_EPOCH).ok()?.as_millis() as u64)
        }
        fn add_file(&self, name: &str, data: Vec<u8>) -> Result<(), String> {
            let p = self.path(name).ok_or("bad path")?;
            std::fs::write(&p, data).map_err(|e| e.to_string())
        }
        fn read_file(&self, name: &str) -> Option<Vec<u8>> {
            std::fs::read(self.path(name)?).ok()
        }
        fn file_names(&self) -> Vec<String> { self.list("") }
    }
}
