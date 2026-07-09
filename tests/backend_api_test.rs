#![cfg(feature = "gfx")]

// The present seam is a Presenter trait, not a headless bool: a backend can be
// injected at runtime and the framebuffer drives it. A mock proves present() is
// called with the committed pixels, input is injected via poll, and the alive
// signal propagates — the same contract minifp/web backends satisfy.

use std::cell::RefCell;
use std::rc::Rc;

use posara::backend::Presenter;
use posara::Framebuffer;

struct MockPresenter {
    presents: Rc<RefCell<Vec<Vec<u16>>>>,
    configured: Rc<RefCell<Option<(usize, usize)>>>,
    inject: Option<(u8, u8)>,
    alive: bool,
}

impl Presenter for MockPresenter {
    fn configure(&mut self, w: usize, h: usize) -> Result<(), String> {
        *self.configured.borrow_mut() = Some((w, h));
        Ok(())
    }
    fn present(&mut self, buf: &[u16]) -> Result<bool, String> {
        self.presents.borrow_mut().push(buf.to_vec());
        Ok(self.alive)
    }
    fn poll(&mut self) -> Option<(u8, u8)> { self.inject }
}

#[test]
fn presenter_is_injectable_and_driven() {
    let presents = Rc::new(RefCell::new(Vec::new()));
    let configured = Rc::new(RefCell::new(None));
    let mut fb = Framebuffer::new();
    fb.set_presenter(Box::new(MockPresenter {
        presents: Rc::clone(&presents),
        configured: Rc::clone(&configured),
        inject: Some((0x11, b'q')),
        alive: true,
    }));

    fb.configure(2, 2, 1).unwrap();
    assert_eq!(*configured.borrow(), Some((2, 2)), "configure not forwarded");

    fb.cls(0x0007);
    fb.commit().unwrap();
    assert_eq!(presents.borrow().len(), 1, "present not called on commit");
    assert_eq!(presents.borrow()[0], vec![0x0007u16; 4], "present got wrong pixels");

    assert_eq!(fb.poll_input(), Some((0x11, b'q')), "input not injected via poll");
}

#[test]
fn dead_surface_flips_alive() {
    let mut fb = Framebuffer::new();
    fb.set_presenter(Box::new(MockPresenter {
        presents: Rc::new(RefCell::new(Vec::new())),
        configured: Rc::new(RefCell::new(None)),
        inject: None,
        alive: false,
    }));
    fb.configure(2, 2, 1).unwrap();
    assert!(fb.alive);
    fb.commit().unwrap();
    assert!(!fb.alive, "dead surface should clear alive");
}

#[test]
fn headless_has_no_presenter() {
    let mut fb = Framebuffer::new();
    fb.configure(2, 2, 1).unwrap();
    fb.commit().unwrap();
    assert_eq!(fb.poll_input(), None, "no presenter → no input");
    assert!(fb.alive);
}
