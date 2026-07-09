// Present seam: framebuffer owns pixels, a Presenter backs the surface.
#[cfg(feature = "gfx-desktop")]
pub mod minifb;

mod clock;
pub use clock::{Clock, SystemClock};

pub mod storage;
pub use storage::{MemStorage, Storage};
#[cfg(feature = "storage-disk")]
pub use storage::DiskStorage;

pub trait Presenter {
    fn configure(&mut self, w: usize, h: usize) -> Result<(), String>;
    // push RGB565 frame; returns surface-alive.
    fn present(&mut self, buf: &[u16]) -> Result<bool, String>;
    fn poll(&mut self) -> Option<(u8, u8)> { None }
    fn set_pos(&mut self, _x: i64, _y: i64) {}
}

use std::path::PathBuf;
use std::rc::Rc;

// One platform bundle. The desktop/web choice lives here — not scattered across
// plugins. Host assembles from a Backend; capability code sees only the traits.
pub struct Backend {
    pub clock: Rc<dyn Clock>,
    pub storage: Rc<dyn Storage>,
}

impl Backend {
    pub fn new(root: PathBuf) -> Self {
        let _ = &root;
        Self {
            clock: Rc::new(SystemClock::new()),
            #[cfg(feature = "storage-disk")]
            storage: Rc::new(DiskStorage::new(root)),
            #[cfg(not(feature = "storage-disk"))]
            storage: Rc::new(MemStorage::new()),
        }
    }

    // The surface backend for a screen. None = headless (render to buf only).
    pub fn presenter(&self, headless: bool) -> Option<Box<dyn Presenter>> {
        let _ = headless;
        #[cfg(feature = "gfx-desktop")]
        if !headless {
            return Some(Box::new(minifb::MinifbPresenter::new()));
        }
        None
    }
}
