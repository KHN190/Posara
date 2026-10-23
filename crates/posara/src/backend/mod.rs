// Present seam: framebuffer owns pixels, a Presenter backs the surface.
#[cfg(feature = "gfx-desktop")]
pub mod minifb;

mod clock;
pub use clock::{Clock, SystemClock};

pub mod storage;
pub use storage::{MemStorage, Storage};
#[cfg(feature = "storage-disk")]
pub use storage::DiskStorage;

pub use posara_core::drivers::Presenter;

#[cfg(feature = "imu")]
use posara_core::drivers::{Imu, NullImu};
#[cfg(feature = "midi")]
use posara_core::drivers::{MidiPort, NullMidi};
#[cfg(feature = "imu")]
use posara_core::imu::SharedImu;
#[cfg(feature = "midi")]
use posara_core::midi::SharedMidi;
#[cfg(any(feature = "imu", feature = "midi"))]
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

// One platform bundle. The desktop/web choice lives here — not scattered across
// plugins. Host assembles from a Backend; capability code sees only the traits.
pub struct Backend {
    pub clock: Rc<dyn Clock>,
    pub storage: Rc<dyn Storage>,
    #[cfg(feature = "imu")]
    pub imu: SharedImu,
    // None = desktop midir routing.
    #[cfg(feature = "midi")]
    pub midi: Option<SharedMidi>,
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
            #[cfg(feature = "imu")]
            imu: Rc::new(RefCell::new(Box::new(NullImu))),
            #[cfg(feature = "midi")]
            midi: None,
        }
    }

    pub fn null(root: PathBuf) -> Self {
        Self {
            #[cfg(feature = "midi")]
            midi: Some(Rc::new(RefCell::new(Box::new(NullMidi)))),
            ..Self::new(root)
        }
    }

    #[cfg(feature = "imu")]
    pub fn imu(self, imu: Box<dyn Imu>) -> Self {
        Self { imu: Rc::new(RefCell::new(imu)), ..self }
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

#[cfg(feature = "midi")]
#[derive(Clone, Default)]
pub struct QueueMidi {
    inbox: Rc<RefCell<std::collections::VecDeque<u32>>>,
    outbox: Rc<RefCell<Vec<u8>>>,
}

#[cfg(feature = "midi")]
impl QueueMidi {
    pub fn push(&self, w: u32) { self.inbox.borrow_mut().push_back(w); }
    pub fn take_bytes(&self) -> Vec<u8> { std::mem::take(&mut *self.outbox.borrow_mut()) }
}

#[cfg(feature = "midi")]
impl MidiPort for QueueMidi {
    fn recv(&mut self) -> Option<u32> { self.inbox.borrow_mut().pop_front() }

    fn send(&mut self, w: u32) -> Result<(), String> {
        let (bytes, len) = posara_core::midi::message(w);
        self.outbox.borrow_mut().extend_from_slice(&bytes[..len]);
        Ok(())
    }
}
