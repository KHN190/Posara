use crate::prelude::*;

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

pub use mem::MemStorage;

mod mem {
    use super::*;
    use core::cell::RefCell;
    use alloc::collections::BTreeMap;

    struct OpenFile { path: String, cursor: usize, writable: bool }

    #[derive(Default)]
    pub struct MemStorage {
        files: RefCell<BTreeMap<String, Vec<u8>>>,
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
            match usize::try_from(fd).ok().and_then(|i| self.open.borrow_mut().get_mut(i).map(core::mem::take)) {
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
