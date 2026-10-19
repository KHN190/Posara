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
    use alloc::collections::BTreeMap;
    use alloc::rc::Rc;
    use core::cell::RefCell;

    type Inode = Rc<RefCell<Vec<u8>>>;

    struct OpenFile { file: Inode, cursor: usize, read: bool, write: bool, append: bool }

    #[derive(Default)]
    pub struct MemStorage {
        files: RefCell<BTreeMap<String, Inode>>,
        open: RefCell<Vec<Option<OpenFile>>>,
    }

    impl MemStorage {
        pub fn new() -> Self { Self::default() }
    }

    fn norm(p: &str) -> String { p.trim_start_matches("./").to_string() }

    impl Storage for MemStorage {
        fn open(&self, path: &str, flags: i64) -> i64 {
            let (read, append) = (flags & F_READ != 0, flags & F_APPEND != 0);
            let write = flags & F_WRITE != 0 || append;
            let (create, trunc) = (flags & F_CREATE != 0, flags & F_TRUNC != 0 && !append);
            if (!read && !write) || (!write && (create || trunc)) { return -1; }
            let file = match self.files.borrow().get(&norm(path)) {
                Some(f) => Rc::clone(f),
                None if create => Inode::default(),
                None => return -1,
            };
            self.files.borrow_mut().entry(norm(path)).or_insert_with(|| Rc::clone(&file));
            if trunc { file.borrow_mut().clear(); }
            let of = OpenFile { file, cursor: 0, read, write, append };
            let mut o = self.open.borrow_mut();
            match o.iter().position(Option::is_none) {
                Some(i) => { o[i] = Some(of); i as i64 }
                None => { o.push(Some(of)); o.len() as i64 - 1 }
            }
        }
        fn close(&self, fd: i64) -> bool {
            usize::try_from(fd).ok().and_then(|i| self.open.borrow_mut().get_mut(i).map(core::mem::take)).is_some_and(|f| f.is_some())
        }
        fn seek(&self, fd: i64, whence: i64, off: i64) -> i64 {
            let mut o = self.open.borrow_mut();
            let Some(Some(of)) = usize::try_from(fd).ok().and_then(|i| o.get_mut(i)) else { return -1; };
            let len = of.file.borrow().len() as i64;
            let pos = match whence { 0 => off.max(0), 1 => of.cursor as i64 + off, 2 => len + off, _ => return -1 };
            if pos < 0 { return -1; }
            of.cursor = pos as usize;
            pos
        }
        fn read(&self, fd: i64, n: usize) -> Option<Vec<u8>> {
            let mut o = self.open.borrow_mut();
            let of = o.get_mut(usize::try_from(fd).ok()?)?.as_mut()?;
            if !of.read { return Some(Vec::new()); }
            let data = of.file.borrow();
            let start = of.cursor.min(data.len());
            let end = start + n.min(data.len() - start);
            let out = data[start..end].to_vec();
            drop(data);
            of.cursor = of.cursor.max(end);
            Some(out)
        }
        fn write(&self, fd: i64, bytes: &[u8]) -> Option<usize> {
            let mut o = self.open.borrow_mut();
            let of = o.get_mut(usize::try_from(fd).ok()?)?.as_mut()?;
            if !of.write { return None; }
            if bytes.is_empty() { return Some(0); }
            let mut data = of.file.borrow_mut();
            if of.append { of.cursor = data.len(); }
            let end = of.cursor + bytes.len();
            if data.len() < end { data.resize(end, 0); }
            data[of.cursor..end].copy_from_slice(bytes);
            of.cursor = end;
            Some(bytes.len())
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
            if !files.contains_key(&key) && files.len() >= MAX_FILES {
                return Err(format!("file limit {}", MAX_FILES));
            }
            let others: usize = files.iter().filter(|(k, _)| **k != key).map(|(_, v)| v.borrow().len()).sum();
            if others + data.len() > MAX_TOTAL_BYTES {
                return Err(format!("total exceeds {} bytes", MAX_TOTAL_BYTES));
            }
            match files.get(&key) {
                Some(f) => *f.borrow_mut() = data,
                None => { files.insert(key, Rc::new(RefCell::new(data))); }
            }
            Ok(())
        }
        fn read_file(&self, name: &str) -> Option<Vec<u8>> {
            self.files.borrow().get(&norm(name)).map(|f| f.borrow().clone())
        }
        fn file_names(&self) -> Vec<String> { self.files.borrow().keys().cloned().collect() }
    }
}
