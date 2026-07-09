// Storage seam. Portable fs natives call Storage; std::fs lives only in
// DiskStorage (desktop). Web gets an in-memory VFS.
use std::path::{Component, Path, PathBuf};

pub const F_READ: i64 = 1;
pub const F_WRITE: i64 = 2;
pub const F_CREATE: i64 = 4;
pub const F_APPEND: i64 = 8;
pub const F_TRUNC: i64 = 16;

// web upload quotas (authoritative in Rust; JS mirrors errors for UX).
pub const MAX_FILES: usize = 16;
pub const MAX_FILE_BYTES: usize = 1 << 20;      // 1 MiB per file
pub const MAX_TOTAL_BYTES: usize = 8 << 20;     // 8 MiB total

pub trait Storage {
    fn open(&self, path: &str, flags: i64) -> i64;
    fn close(&self, fd: i64) -> bool;
    fn seek(&self, fd: i64, whence: i64, off: i64) -> i64;
    fn read(&self, fd: i64, n: usize) -> Option<Vec<u8>>;
    fn write(&self, fd: i64, data: &[u8]) -> Option<usize>;
    fn exists(&self, path: &str) -> bool;
    fn mkdir(&self, path: &str) -> bool;
    fn remove(&self, path: &str) -> bool;
    fn list(&self, path: &str) -> Vec<String>;
    fn mtime(&self, path: &str) -> Option<u64>;
    // upload sandbox API (JS drop/editor → these; cart sees the same set).
    fn add_file(&self, name: &str, data: Vec<u8>) -> Result<(), String>;
    fn read_file(&self, name: &str) -> Option<Vec<u8>>;
    fn file_names(&self) -> Vec<String>;
}

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
            let mut buf = vec![0u8; n];
            let got = f.read(&mut buf).unwrap_or(0);
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

// -------- web / headless: in-memory VFS --------
pub use mem::MemStorage;

mod mem {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;

    struct OpenFile { path: String, cursor: usize, writable: bool }

    #[derive(Default)]
    pub struct MemStorage {
        files: RefCell<HashMap<String, Vec<u8>>>,
        open: RefCell<Vec<Option<OpenFile>>>,
    }
    impl MemStorage {
        pub fn new() -> Self { Self::default() }
        fn slot(&self, path: &str, writable: bool) -> i64 {
            let mut o = self.open.borrow_mut();
            let of = OpenFile { path: norm(path), cursor: 0, writable };
            if let Some(i) = o.iter().position(|s| s.is_none()) { o[i] = Some(of); i as i64 }
            else { o.push(Some(of)); (o.len() - 1) as i64 }
        }
    }
    fn norm(p: &str) -> String { p.trim_start_matches("./").to_string() }

    impl Storage for MemStorage {
        fn open(&self, path: &str, flags: i64) -> i64 {
            let exists = self.files.borrow().contains_key(&norm(path));
            if !exists && flags & F_CREATE == 0 && flags & F_READ != 0 { return -1; }
            if flags & F_CREATE != 0 || (flags & F_TRUNC != 0 && flags & F_APPEND == 0) {
                self.files.borrow_mut().entry(norm(path)).or_default();
                if flags & F_TRUNC != 0 { self.files.borrow_mut().insert(norm(path), Vec::new()); }
            }
            let fd = self.slot(path, flags & (F_WRITE | F_APPEND) != 0);
            if flags & F_APPEND != 0 {
                let len = self.files.borrow().get(&norm(path)).map(|v| v.len()).unwrap_or(0);
                if let Some(Some(of)) = self.open.borrow_mut().get_mut(fd as usize) { of.cursor = len; }
            }
            fd
        }
        fn close(&self, fd: i64) -> bool {
            match usize::try_from(fd).ok().and_then(|i| self.open.borrow_mut().get_mut(i).map(std::mem::take)) {
                Some(Some(_)) => true,
                _ => false,
            }
        }
        fn seek(&self, fd: i64, whence: i64, off: i64) -> i64 {
            let mut o = self.open.borrow_mut();
            let Some(Some(of)) = o.get_mut(fd as usize) else { return -1; };
            let len = self.files.borrow().get(&of.path).map(|v| v.len()).unwrap_or(0) as i64;
            let pos = match whence { 0 => off, 1 => of.cursor as i64 + off, 2 => len + off, _ => return -1 };
            if pos < 0 { return -1; }
            of.cursor = pos as usize;
            pos
        }
        fn read(&self, fd: i64, n: usize) -> Option<Vec<u8>> {
            let mut o = self.open.borrow_mut();
            let of = o.get_mut(fd as usize)?.as_mut()?;
            let files = self.files.borrow();
            let data = files.get(&of.path)?;
            let start = of.cursor.min(data.len());
            let end = (start + n).min(data.len());
            of.cursor = end;
            Some(data[start..end].to_vec())
        }
        fn write(&self, fd: i64, data: &[u8]) -> Option<usize> {
            let mut o = self.open.borrow_mut();
            let of = o.get_mut(fd as usize)?.as_mut()?;
            if !of.writable { return None; }
            let mut files = self.files.borrow_mut();
            let buf = files.entry(of.path.clone()).or_default();
            let at = of.cursor;
            if buf.len() < at { buf.resize(at, 0); }
            let tail = &mut buf[at..];
            let overlap = tail.len().min(data.len());
            tail[..overlap].copy_from_slice(&data[..overlap]);
            buf.extend_from_slice(&data[overlap..]);
            of.cursor = at + data.len();
            Some(data.len())
        }
        fn exists(&self, path: &str) -> bool { self.files.borrow().contains_key(&norm(path)) }
        fn mkdir(&self, _path: &str) -> bool { true }
        fn remove(&self, path: &str) -> bool { self.files.borrow_mut().remove(&norm(path)).is_some() }
        fn list(&self, _path: &str) -> Vec<String> { self.file_names() }
        fn mtime(&self, _path: &str) -> Option<u64> { None }

        fn add_file(&self, name: &str, data: Vec<u8>) -> Result<(), String> {
            if data.len() > MAX_FILE_BYTES {
                return Err(format!("{} exceeds {} bytes", name, MAX_FILE_BYTES));
            }
            let mut files = self.files.borrow_mut();
            let key = norm(name);
            let is_new = !files.contains_key(&key);
            if is_new && files.len() >= MAX_FILES {
                return Err(format!("file limit {}", MAX_FILES));
            }
            let others: usize = files.iter().filter(|(k, _)| **k != key).map(|(_, v)| v.len()).sum();
            if others + data.len() > MAX_TOTAL_BYTES {
                return Err(format!("total exceeds {} bytes", MAX_TOTAL_BYTES));
            }
            files.insert(key, data);
            Ok(())
        }
        fn read_file(&self, name: &str) -> Option<Vec<u8>> {
            self.files.borrow().get(&norm(name)).cloned()
        }
        fn file_names(&self) -> Vec<String> {
            let mut v: Vec<String> = self.files.borrow().keys().cloned().collect();
            v.sort();
            v
        }
    }
}
