use core::cell::{Cell, RefCell};

use myriad::{read_string, NativeCtx, Value, VirtualMachine};

use crate::abi::Args;
use crate::drivers::{Clock, NullClock};
use crate::plugin::Plugin;
use crate::prelude::*;

#[cfg(feature = "gfx")]
use crate::drivers::Presenter;
#[cfg(feature = "gfx")]
use crate::gfx::{input::InputPlugin, GfxPlugin};
#[cfg(feature = "sfx")]
use crate::sfx::natives::{self, CmdProd};
#[cfg(feature = "fs")]
use crate::fs::{FsPlugin, MemStorage, Storage};
#[cfg(feature = "midi")]
use crate::midi::{MidiPlugin, SharedMidi};
#[cfg(feature = "net")]
use crate::net::{NetPlugin, NullSockets, Sockets};
#[cfg(feature = "imu")]
use crate::imu::{ImuPlugin, SharedImu};

pub struct Drivers {
    pub clock: Rc<dyn Clock>,
    #[cfg(feature = "gfx")]
    pub display: Option<Box<dyn Presenter>>,
    #[cfg(feature = "sfx")]
    pub cmds: CmdProd,
    #[cfg(feature = "fs")]
    pub storage: Rc<dyn Storage>,
    #[cfg(feature = "midi")]
    pub midi: SharedMidi,
    #[cfg(feature = "net")]
    pub sockets: Box<dyn Sockets>,
    #[cfg(feature = "imu")]
    pub imu: SharedImu,
}

impl Drivers {
    pub fn null() -> Self {
        Self {
            clock: Rc::new(NullClock),
            #[cfg(feature = "gfx")]
            display: None,
            #[cfg(feature = "sfx")]
            cmds: alloc::sync::Arc::new(crate::sfx::spsc::Spsc::new(1024)),
            #[cfg(feature = "fs")]
            storage: Rc::new(MemStorage::new()),
            #[cfg(feature = "midi")]
            midi: Rc::new(RefCell::new(Box::new(crate::drivers::NullMidi))),
            #[cfg(feature = "net")]
            sockets: Box::new(NullSockets::new()),
            #[cfg(feature = "imu")]
            imu: Rc::new(RefCell::new(Box::new(crate::drivers::NullImu))),
        }
    }
}

pub struct Host {
    pub clock_src: Rc<dyn Clock>,
    // Some(ms) = virtual time for offline, deterministic runs.
    pub clock: Rc<Cell<Option<u64>>>,
    pub rng: Rc<RefCell<u32>>,
    // INT32_SAFE cart: Float values travel as f32 bits.
    pub float32: Rc<Cell<bool>>,
    #[cfg(feature = "gfx")]
    pub gfx: GfxPlugin,
    #[cfg(feature = "gfx")]
    pub input: InputPlugin,
    #[cfg(feature = "sfx")]
    pub cmds: CmdProd,
    #[cfg(feature = "fs")]
    pub fs: FsPlugin,
    #[cfg(feature = "midi")]
    pub midi: MidiPlugin,
    #[cfg(feature = "net")]
    pub net: NetPlugin,
    #[cfg(feature = "imu")]
    pub imu: ImuPlugin,
}

impl Host {
    pub fn new(d: Drivers) -> Self {
        #[cfg(feature = "gfx")]
        let gfx = GfxPlugin::new(d.display);
        Self {
            clock_src: d.clock,
            clock: Rc::new(Cell::new(None)),
            rng: Rc::new(RefCell::new(0x9e3779b9)),
            float32: Rc::new(Cell::new(false)),
            #[cfg(feature = "gfx")]
            input: InputPlugin::new(Rc::clone(&gfx.fb)),
            #[cfg(feature = "gfx")]
            gfx,
            #[cfg(feature = "sfx")]
            cmds: d.cmds,
            #[cfg(feature = "fs")]
            fs: FsPlugin::new(d.storage),
            #[cfg(feature = "midi")]
            midi: MidiPlugin { port: d.midi },
            #[cfg(feature = "net")]
            net: NetPlugin::new(d.sockets),
            #[cfg(feature = "imu")]
            imu: ImuPlugin { imu: d.imu },
        }
    }

    pub fn load(&self, module: &polka::Module) {
        self.float32.set(module.flags & polka::CART_FLAG_INT32_SAFE != 0);
        #[cfg(feature = "net")]
        self.net.sweep();
    }

    pub fn now_ms(&self) -> u64 {
        self.clock.get().unwrap_or_else(|| self.clock_src.now_ms())
    }

    fn plugins(&self) -> Vec<&dyn Plugin> {
        #[allow(unused_mut)]
        let mut v: Vec<&dyn Plugin> = Vec::new();
        #[cfg(feature = "gfx")]
        v.push(&self.gfx);
        #[cfg(feature = "gfx")]
        v.push(&self.input);
        #[cfg(feature = "fs")]
        v.push(&self.fs);
        #[cfg(feature = "midi")]
        v.push(&self.midi);
        #[cfg(feature = "net")]
        v.push(&self.net);
        #[cfg(feature = "imu")]
        v.push(&self.imu);
        v
    }

    pub fn install(&self, vm: &mut VirtualMachine) {
        crate::abi::stub_all(vm);
        #[cfg(not(feature = "gfx"))]
        {
            vm.install_device(crate::devices::SCREEN, Box::new(StubDevice::new("Screen", "gfx")));
            vm.install_device(crate::devices::CONTROLLER, Box::new(StubDevice::new("Controller", "gfx")));
        }
        #[cfg(not(feature = "midi"))]
        vm.install_device(crate::devices::MIDI, Box::new(StubDevice::new("MIDI", "midi")));
        for p in self.plugins() { p.install(vm); }
        #[cfg(feature = "sfx")]
        natives::register(vm, alloc::sync::Arc::clone(&self.cmds));
        #[cfg(feature = "synth")]
        natives::register_synth(vm, alloc::sync::Arc::clone(&self.cmds));
        self.register_core_natives(vm);
    }

    #[cfg(feature = "gfx")]
    pub fn set_input(&self, buttons: u8, key: u8) {
        self.input.controller.borrow_mut().set(buttons, key);
    }

    fn register_core_natives(&self, vm: &mut VirtualMachine) {
        let (clock, src) = (Rc::clone(&self.clock), Rc::clone(&self.clock_src));
        vm.register_native("now", Rc::new(move |_: &mut NativeCtx, _: &[Value]| {
            let ms = clock.get().unwrap_or_else(|| src.now_ms());
            Ok((Value::from_int(ms as i64), false))
        }));
        let (r, f32_bits) = (Rc::clone(&self.rng), Rc::clone(&self.float32));
        vm.register_native("rand", Rc::new(move |_: &mut NativeCtx, _: &[Value]| {
            let mut s = r.borrow_mut();
            let mut x = if *s == 0 { 0x9e3779b9 } else { *s };
            x ^= x << 13; x ^= x >> 17; x ^= x << 5;
            *s = x;
            let f = (x as f64) / (u32::MAX as f64);
            Ok((Value::from_raw(myriad::encode_float(f, f32_bits.get())), false))
        }));
        let r = Rc::clone(&self.rng);
        vm.register_native("srand", Rc::new(move |_: &mut NativeCtx, a: &[Value]| {
            let seed = Args(a).word(0) as u32;
            *r.borrow_mut() = if seed == 0 { 0x9e3779b9 } else { seed };
            Ok((Value::UNIT, false))
        }));
        vm.register_native("assert", Rc::new(|ctx: &mut NativeCtx, a: &[Value]| {
            if Args(a).flag(0) {
                return Ok((Value::UNIT, false));
            }
            let msg = read_string(ctx.heap, Args(a).handle(1)).unwrap_or_default();
            Err(format!("assert failed: {msg}"))
        }));
    }
}

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
            crate::warn!("• {}: posara built without `{}` feature; events dropped", self.name, self.feature);
        }
    }
}

#[cfg(any(not(feature = "midi"), not(feature = "gfx")))]
impl myriad::Device for StubDevice {
    fn read(&mut self, _port: u8) -> Result<(Value, bool), String> {
        self.warn();
        Ok((Value::ZERO, false))
    }

    fn write(&mut self, _port: u8, _val: Value, _is_handle: bool, _heap: &mut myriad::Heap) -> Result<(), String> {
        self.warn();
        Ok(())
    }
}
