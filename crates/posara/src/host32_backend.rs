use std::{path::PathBuf, rc::Rc};
use posara_core::{word32 as narrow, word64 as wide};
use narrow::drivers::{Clock, Presenter};
use narrow::fs::Storage;

pub struct ClockAdapter(pub Rc<dyn wide::drivers::Clock>);
impl Clock for ClockAdapter {
    fn now_ms(&self) -> u32 { self.0.now_ms() as u32 }
    fn reset(&self) { self.0.reset(); }
}

pub struct StorageAdapter(pub Rc<dyn wide::fs::Storage>);
impl Storage for StorageAdapter {
    fn open(&self, path: &str, flags: i32) -> i32 {
        let fd = self.0.open(path, flags as i64);
        i32::try_from(fd).unwrap_or_else(|_| { self.0.close(fd); -1 })
    }
    fn close(&self, fd: i32) -> bool { self.0.close(fd as i64) }
    fn seek(&self, fd: i32, whence: i32, off: i32) -> i32 {
        let old = self.0.seek(fd as i64, 1, 0);
        let result = self.0.seek(fd as i64, whence as i64, off as i64);
        i32::try_from(result).unwrap_or_else(|_| { if old >= 0 { self.0.seek(fd as i64, 0, old); } -1 })
    }
    fn read(&self, fd: i32, n: usize) -> Option<Vec<u8>> { self.0.read(fd as i64, n) }
    fn write(&self, fd: i32, data: &[u8]) -> Option<usize> { self.0.write(fd as i64, data) }
    fn exists(&self, path: &str) -> bool { self.0.exists(path) }
    fn mkdir(&self, path: &str) -> bool { self.0.mkdir(path) }
    fn remove(&self, path: &str) -> bool { self.0.remove(path) }
    fn list(&self, path: &str) -> Vec<String> { self.0.list(path) }
    fn mtime(&self, path: &str) -> Option<u32> { u32::try_from(self.0.mtime(path)?).ok() }
    fn add_file(&self, name: &str, data: Vec<u8>) -> Result<(), String> { self.0.add_file(name, data) }
    fn read_file(&self, name: &str) -> Option<Vec<u8>> { self.0.read_file(name) }
    fn file_names(&self) -> Vec<String> { self.0.file_names()}
}

pub enum PresenterAdapter {
    Raw(Box<dyn wide::drivers::Presenter>),
    #[cfg(feature = "gfx")]
    Framebuffer(Rc<std::cell::RefCell<wide::gfx::Framebuffer>>),
}

impl Presenter for PresenterAdapter {
    fn configure(&mut self, w: usize, h: usize) -> Result<(), String> {
        match self {
            Self::Raw(p) => p.configure(w, h),
            #[cfg(feature = "gfx")]
            Self::Framebuffer(fb) => fb.borrow_mut().configure(w, h, 1),
        }
    }
    fn present(&mut self, buf: &[u16]) -> Result<bool, String> {
        match self {
            Self::Raw(p) => p.present(buf),
            #[cfg(feature = "gfx")]
            Self::Framebuffer(fb) => {
                let mut fb = fb.borrow_mut();
                if fb.buf.len() != buf.len() {
                    fb.buf = buf.to_vec();
                } else {
                    fb.buf.copy_from_slice(buf);
                }
                fb.commit()?;
                Ok(fb.alive)
            }
        }
    }
    fn poll(&mut self) -> Option<(u8, u8)> {
        match self {
            Self::Raw(p) => p.poll(),
            #[cfg(feature = "gfx")]
            Self::Framebuffer(fb) => fb.borrow_mut().poll_input(),
        }
    }
    fn set_pos(&mut self, x: i32, y: i32) {
        match self {
            Self::Raw(p) => p.set_pos(x as i64, y as i64),
            #[cfg(feature = "gfx")]
            Self::Framebuffer(fb) => fb.borrow_mut().win_pos(x as i64, y as i64),
        }
    }
}

#[cfg(feature = "net")]
pub struct SocketsAdapter(pub Box<dyn wide::net::Sockets>);

#[cfg(feature = "net")]
impl narrow::net::Sockets for SocketsAdapter {
    fn bind(&mut self, port: u16) -> i32 { let id = self.0.bind(port); i32::try_from(id).unwrap_or_else(|_| { self.0.close(id); -1 }) }
    fn addr(&self, id: i32) -> Option<String> { self.0.addr(id as i64) }
    fn recv(&mut self, id: i32) -> Option<usize> { self.0.recv(id as i64) }
    fn recv_buf(&self) -> &[u8] { self.0.recv_buf() }
    fn from(&self, id: i32) -> Option<String> { self.0.from(id as i64) }
    fn send(&self, id: i32, addr: &str, data: &[u8]) -> Option<()> { self.0.send(id as i64, addr, data) }
    fn broadcast(&self, id: i32, on: bool) -> Option<()> { self.0.broadcast(id as i64, on) }
    fn close(&mut self, id: i32) { self.0.close(id as i64) }
    fn sweep(&mut self) { self.0.sweep() }
    fn live(&self) -> usize { self.0.live() }
}

pub struct Backend {
    os: crate::backend::Backend,
    pub clock: Rc<dyn Clock>,
    pub storage: Rc<dyn Storage>,
    #[cfg(feature = "midi")]
    pub midi: Option<narrow::midi::SharedMidi>,
    #[cfg(feature = "imu")]
    pub imu: narrow::imu::SharedImu,
}

impl Backend {
    pub fn new(root: PathBuf) -> Self { Self::from_os(crate::backend::Backend::new(root)) }
    pub fn null(root: PathBuf) -> Self { Self::from_os(crate::backend::Backend::null(root)) }
    pub fn from_os(os: crate::backend::Backend) -> Self {
        Self {
            clock: Rc::new(ClockAdapter(Rc::clone(&os.clock))),
            storage: Rc::new(StorageAdapter(Rc::clone(&os.storage))),
            #[cfg(feature = "midi")]
            midi: os.midi.clone(),
            #[cfg(feature = "imu")]
            imu: os.imu.clone(),
            os,
        }
    }
    pub fn presenter(&self, headless: bool) -> Option<Box<dyn Presenter>> {
        self.os.presenter(headless).map(|p| Box::new(PresenterAdapter::Raw(p)) as Box<dyn Presenter>)
    }
    #[cfg(feature = "net")]
    pub fn sockets(&self) -> Box<dyn narrow::net::Sockets> {
        Box::new(SocketsAdapter(Box::new(crate::plugins::net::SocketTable::new())))
    }
}
