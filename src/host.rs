use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Instant;

use myriad::{NativeCtx, Value, VirtualMachine};

#[cfg(feature = "gfx")]
use crate::plugins::GfxPlugin;
#[cfg(feature = "sfx")]
use crate::plugins::SfxPlugin;
#[cfg(feature = "synth")]
use crate::plugins::SynthPlugin;
#[cfg(feature = "fs")]
use crate::plugins::FsPlugin;
#[cfg(feature = "midi")]
use crate::plugins::MidiPlugin;
use crate::plugin::Plugin;

// Every capability is a Plugin (install + register_fns). Host keeps typed
// fields where callers need direct access (gfx framebuffer, sfx recorder) and
// iterates them uniformly through the trait for VM/compiler wiring.
pub struct Host {
    pub start: Instant,
    // When Some(ms), now() returns this virtual time instead of wall clock —
    // offline rendering drives it so timing is deterministic and non-realtime.
    pub clock: Rc<std::cell::Cell<Option<u64>>>,
    pub rng: Rc<RefCell<u32>>,
    pub root: PathBuf,
    #[cfg(feature = "gfx")]
    pub gfx: GfxPlugin,
    #[cfg(feature = "sfx")]
    pub sfx: SfxPlugin,
    #[cfg(feature = "synth")]
    synth: SynthPlugin,
    #[cfg(feature = "fs")]
    pub fs: FsPlugin,
    #[cfg(feature = "midi")]
    midi: MidiPlugin,
}

impl Host {
    pub fn new(root: PathBuf) -> Result<Self, String> { Self::new_with(root, false, false) }

    pub fn new_with(root: PathBuf, headless: bool, muted: bool) -> Result<Self, String> {
        Self::build(root, headless, muted, |_root| {
            #[cfg(feature = "midi")]
            return MidiPlugin::new();
        })
    }

    // Cart-aware constructor: routes MIDI per `<root>/midi.toml` (see
    // designs/midi.md). Without a config this is `new_with`. muted = no audio
    // device (silent run, like --headless for sound).
    pub fn new_cart(root: PathBuf, headless: bool, muted: bool, entry: &std::path::Path) -> Result<Self, String> {
        #[cfg(not(feature = "midi"))]
        let _ = entry;
        Self::build(root, headless, muted, |_root| {
            #[cfg(feature = "midi")]
            return MidiPlugin::new_routed(_root, entry);
        })
    }

    #[cfg(feature = "midi")]
    fn build(root: PathBuf, headless: bool, muted: bool, midi: impl FnOnce(&std::path::Path) -> MidiPlugin) -> Result<Self, String> {
        let midi = midi(&root);
        Self::assemble(root, headless, muted, midi)
    }

    #[cfg(not(feature = "midi"))]
    fn build(root: PathBuf, headless: bool, muted: bool, _midi: impl FnOnce(&std::path::Path)) -> Result<Self, String> {
        Self::assemble(root, headless, muted)
    }

    fn assemble(root: PathBuf, headless: bool, muted: bool, #[cfg(feature = "midi")] midi: MidiPlugin) -> Result<Self, String> {
        #[cfg(not(feature = "gfx"))]
        let _ = headless;
        #[cfg(not(feature = "sfx"))]
        let _ = muted;
        #[cfg(feature = "sfx")]
        let sfx = SfxPlugin::with_audio(root.clone(), muted)?;
        Ok(Self {
            start: Instant::now(),
            clock: Rc::new(std::cell::Cell::new(None)),
            rng: Rc::new(RefCell::new(0x9e3779b9)),
            #[cfg(feature = "gfx")]
            gfx: GfxPlugin::new(headless, root.clone()),
            #[cfg(feature = "synth")]
            synth: SynthPlugin::new(std::sync::Arc::clone(&sfx.audio.cmds)),
            #[cfg(feature = "sfx")]
            sfx,
            #[cfg(feature = "fs")]
            fs: FsPlugin::new(root.clone()),
            #[cfg(feature = "midi")]
            midi,
            root,
        })
    }

    fn plugin_list(&self) -> Vec<&dyn Plugin> {
        let mut v: Vec<&dyn Plugin> = Vec::new();
        #[cfg(feature = "gfx")]
        v.push(&self.gfx);
        #[cfg(feature = "sfx")]
        v.push(&self.sfx);
        #[cfg(feature = "synth")]
        v.push(&self.synth);
        #[cfg(feature = "fs")]
        v.push(&self.fs);
        #[cfg(feature = "midi")]
        v.push(&self.midi);
        v
    }

    pub fn install(&self, vm: &mut VirtualMachine) {
        for p in self.plugin_list() { p.install(vm); }
        #[cfg(not(feature = "midi"))]
        vm.install_device(0x90, Box::new(StubDevice::new("MIDI", "midi")));
        #[cfg(not(feature = "gfx"))]
        {
            vm.install_device(0x20, Box::new(StubDevice::new("Screen", "gfx")));
            vm.install_device(0x80, Box::new(StubDevice::new("Controller", "gfx")));
        }
        register_time_natives(vm, self.start, Rc::clone(&self.clock));
        register_rand_natives(vm, Rc::clone(&self.rng));
        #[cfg(feature = "test")]
        crate::debug::register_assert_natives(vm);
    }

    #[cfg(feature = "gfx")]
    pub fn set_input(&self, buttons: u8, key: u8) {
        let mut c = self.gfx.controller.borrow_mut();
        c.buttons = buttons;
        c.key = key;
    }

    #[cfg(feature = "compiler")]
    pub fn register_host_fns(&self, compiler: &mut abrase::compiler::Compiler) -> Result<(), String> {
        use abrase::ty::Type as T;
        use abrase::ast::EffectItem;
        let io_eff = || vec![EffectItem { name: vec!["IO".into()], arg: None }];

        for p in self.plugin_list() { p.register_fns(compiler)?; }

        compiler.register_host_fn("now",   vec![],       T::Int,   io_eff())?;
        compiler.register_host_fn("rand",  vec![],       T::Float, vec![EffectItem { name: vec!["nondet".into()], arg: None }])?;
        compiler.register_host_fn("srand", vec![T::Int], T::Unit,  vec![EffectItem { name: vec!["nondet".into()], arg: None }])?;
        #[cfg(feature = "test")]
        {
            compiler.register_host_fn("assert", vec![T::Bool, T::String], T::Unit, vec![])?;
        }
        Ok(())
    }
}

fn ret_unit() -> Result<(Value, bool), String> { Ok((Value::UNIT, false)) }

#[cfg(any(not(feature = "midi"), not(feature = "gfx")))]
struct StubDevice {
    name: &'static str,
    feature: &'static str,
    warned: bool,
}

#[cfg(any(not(feature = "midi"), not(feature = "gfx")))]
impl StubDevice {
    fn new(name: &'static str, feature: &'static str) -> Self {
        Self { name, feature, warned: false }
    }

    fn warn(&mut self) {
        if !self.warned {
            self.warned = true;
            eprintln!("• {}: posara built without `{}` feature; events dropped", self.name, self.feature);
        }
    }
}

#[cfg(any(not(feature = "midi"), not(feature = "gfx")))]
impl myriad::Device for StubDevice {
    fn read(&mut self, _port: u8) -> Result<(Value, bool), String> {
        self.warn();
        Ok((Value::ZERO, false))
    }

    fn write(&mut self, _port: u8, _val: Value, _is_handle: bool, _heap: &mut myriad::memory::Heap) -> Result<(), String> {
        self.warn();
        Ok(())
    }
}

fn register_time_natives(vm: &mut VirtualMachine, start: Instant, clock: Rc<std::cell::Cell<Option<u64>>>) {
    vm.register_native("now", Rc::new(move |_: &mut NativeCtx, _: &[Value]| {
        let ms = clock.get().unwrap_or_else(|| start.elapsed().as_millis() as u64);
        Ok((Value::from_int(ms as i64), false))
    }));
}

fn register_rand_natives(vm: &mut VirtualMachine, rng: Rc<RefCell<u32>>) {
    let r = Rc::clone(&rng);
    vm.register_native("rand", Rc::new(move |_: &mut NativeCtx, _: &[Value]| {
        let mut s = r.borrow_mut();
        let mut x = if *s == 0 { 0x9e3779b9 } else { *s };
        x ^= x << 13; x ^= x >> 17; x ^= x << 5;
        *s = x;
        let f = (x as f64) / (u32::MAX as f64);
        Ok((Value::from_float(f), false))
    }));
    let r = Rc::clone(&rng);
    vm.register_native("srand", Rc::new(move |_: &mut NativeCtx, a: &[Value]| {
        let seed = a.first().copied().unwrap_or(Value::ZERO).as_int() as u32;
        *r.borrow_mut() = if seed == 0 { 0x9e3779b9 } else { seed };
        ret_unit()
    }));
}
