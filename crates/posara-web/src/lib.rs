use std::path::PathBuf;

use posara::backend::Storage;
use posara::runner::{compile_source, compile_source_multi, read_pk_bytes, Stepper};
use posara::Host;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
}

// host boxed (stable address); stepper listed first so it drops before host.
#[wasm_bindgen]
pub struct Posara {
    stepper: Option<Stepper<'static>>,
    host: Box<Host>,
}

#[wasm_bindgen]
impl Posara {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<Posara, JsValue> {
        let host = Host::new_with(PathBuf::from("."), false, false).map_err(js)?;
        Ok(Posara { stepper: None, host: Box::new(host) })
    }

    pub fn load_src(&mut self, src: &str) -> Result<(), JsValue> {
        let r = compile_source(src, &self.host).map_err(js)?;
        self.start(r.module, r.static_names, r.fn_names)
    }

    // multi-module: compiles the named .abe, resolving `use a::b` → a/b.abe from
    // the upload sandbox; missing modules error.
    pub fn load_entry(&mut self, name: &str) -> Result<(), JsValue> {
        let r = compile_source_multi(name, &*self.host.fs.storage, &self.host).map_err(js)?;
        self.start(r.module, r.static_names, r.fn_names)
    }

    pub fn load_pk(&mut self, bytes: &[u8]) -> Result<(), JsValue> {
        let module = read_pk_bytes(bytes).map_err(js)?;
        self.start(module, Vec::new(), Vec::new())
    }

    fn start(
        &mut self,
        module: posara::Module,
        static_names: Vec<String>,
        fn_names: Vec<String>,
    ) -> Result<(), JsValue> {
        self.stepper = None;
        self.host.gfx.fb.borrow_mut().reset();   // allow a new cart to set a different screen size
        let host: &'static Host = unsafe { &*(&*self.host as *const Host) };
        self.stepper = Some(Stepper::start_named(module, static_names, fn_names, host).map_err(js)?);
        Ok(())
    }

    pub fn frame(&mut self) -> Result<bool, JsValue> {
        match self.stepper.as_mut() {
            Some(s) => s.frame().map_err(js),
            None => Ok(false),
        }
    }

    pub fn width(&self) -> u32 { self.host.gfx.fb.borrow().w as u32 }
    pub fn height(&self) -> u32 { self.host.gfx.fb.borrow().h as u32 }

    pub fn framebuffer(&self) -> Vec<u8> {
        self.host.gfx.fb.borrow().out.iter().flat_map(|&px| px.to_le_bytes()).collect()
    }

    pub fn set_input(&mut self, buttons: u8, key: u8) {
        let mut c = self.host.input.controller.borrow_mut();
        c.buttons = buttons;
        c.key = key;
    }

    pub fn audio_pull(&mut self, out: &mut [f32]) {
        self.host.sfx.audio.pull(out);
    }

    pub fn sample_rate(&self) -> u32 { self.host.sfx.audio.sample_rate }
    pub fn steps(&self) -> u64 { self.stepper.as_ref().map(|s| s.steps()).unwrap_or(0) }
    pub fn max_ops(&self) -> u64 { posara::runner::OPS_BUDGET }

    // cart stdout since last drain (println etc). Frontend tags it as cart output.
    pub fn drain_stdout(&self) -> String {
        let mut b = self.host.console_out.borrow_mut();
        if b.is_empty() { return String::new(); }
        let s = String::from_utf8_lossy(&b).into_owned();
        b.clear();
        s
    }

    // upload sandbox — cart sees exactly these files. Quota enforced in Rust.
    pub fn add_file(&mut self, name: &str, data: &[u8]) -> Result<(), JsValue> {
        self.host.fs.storage.add_file(name, data.to_vec()).map_err(js)
    }
    pub fn remove_file(&mut self, name: &str) -> bool {
        self.host.fs.storage.remove(name)
    }
    pub fn list_files(&self) -> Vec<String> {
        self.host.fs.storage.file_names()
    }
    pub fn max_files(&self) -> usize { posara::backend::storage::MAX_FILES }
    pub fn version(&self) -> String { posara::VERSION.to_string() }
    pub fn read_file(&self, name: &str) -> Option<Vec<u8>> {
        self.host.fs.storage.read_file(name)
    }
}

fn js<S: std::fmt::Display>(s: S) -> JsValue {
    JsValue::from_str(&s.to_string())
}
