use std::cell::RefCell;
use std::ops::Deref;
use std::path::PathBuf;
use std::rc::Rc;

use myriad::VirtualMachine;
use posara_core::host::Drivers;

use crate::backend::Backend;
#[cfg(any(feature = "sfx", feature = "midi-desktop"))]
use crate::plugin::Plugin;
#[cfg(feature = "sfx")]
use crate::plugins::SfxPlugin;
#[cfg(feature = "midi-desktop")]
use crate::plugins::MidiPlugin;

// Desktop/web host: the portable core plus what needs std (audio device,
// recorder, PNG, midir routing). Deref exposes the core's plugins.
pub struct Host {
    core: posara_core::host::Host,
    // Cart stdout sink for the web console; the frontend drains it each frame.
    pub console_out: myriad::devices::console::SharedBuf,
    pub root: PathBuf,
    #[cfg(feature = "sfx")]
    pub sfx: SfxPlugin,
    #[cfg(feature = "midi-desktop")]
    midi_io: Option<MidiPlugin>,
}

impl Deref for Host {
    type Target = posara_core::host::Host;
    fn deref(&self) -> &Self::Target { &self.core }
}

#[cfg(feature = "midi-desktop")]
type MidiIo = Option<MidiPlugin>;
#[cfg(not(feature = "midi-desktop"))]
type MidiIo = ();

impl Host {
    pub fn new(root: PathBuf) -> Result<Self, String> { Self::new_with(root, false, false) }

    pub fn new_with(root: PathBuf, headless: bool, muted: bool) -> Result<Self, String> {
        Self::build(root.clone(), Backend::new(root), headless, muted, |_root| {
            #[cfg(feature = "midi-desktop")]
            return Some(MidiPlugin::new());
        })
    }

    pub fn with_backend(root: PathBuf, backend: Backend, headless: bool, muted: bool) -> Result<Self, String> {
        Self::build(root, backend, headless, muted, |_root| {
            #[cfg(feature = "midi-desktop")]
            return None;
        })
    }

    // Cart-aware constructor: routes MIDI per `<root>/midi.toml` (see
    // designs/midi.md). Without a config this is `new_with`. muted = no audio
    // device (silent run, like --headless for sound).
    pub fn new_cart(root: PathBuf, headless: bool, muted: bool, entry: &std::path::Path) -> Result<Self, String> {
        #[cfg(not(feature = "midi-desktop"))]
        let _ = entry;
        Self::build(root.clone(), Backend::new(root), headless, muted, |_root| {
            #[cfg(feature = "midi-desktop")]
            return Some(MidiPlugin::new_routed(_root, entry));
        })
    }

    fn build(root: PathBuf, backend: Backend, headless: bool, muted: bool, midi: impl FnOnce(&std::path::Path) -> MidiIo) -> Result<Self, String> {
        #[cfg(not(feature = "gfx"))]
        let _ = headless;
        #[cfg(not(feature = "sfx"))]
        let _ = muted;
        #[cfg(feature = "midi-desktop")]
        let midi_io = if backend.midi.is_some() { None } else { midi(&root) };
        #[cfg(not(feature = "midi-desktop"))]
        midi(&root);
        #[cfg(feature = "sfx")]
        let sfx = SfxPlugin::with_audio(root.clone(), muted)?;
        let core = posara_core::host::Host::new(Drivers {
            clock: Rc::clone(&backend.clock),
            #[cfg(feature = "gfx")]
            display: backend.presenter(headless),
            #[cfg(feature = "sfx")]
            cmds: std::sync::Arc::clone(&sfx.audio.cmds),
            storage: Rc::clone(&backend.storage),
            #[cfg(feature = "midi")]
            midi: backend.midi.clone().unwrap_or_else(|| Rc::new(RefCell::new(Box::new(posara_core::drivers::NullMidi)))),
            #[cfg(feature = "net")]
            sockets: Box::new(crate::plugins::net::SocketTable::new()),
            #[cfg(feature = "imu")]
            imu: backend.imu,
        });
        // Web pixel budget (memory bound; presenter scales up).
        #[cfg(all(feature = "gfx", not(feature = "gfx-desktop")))]
        {
            let mut fb = core.gfx.fb.borrow_mut();
            fb.max_dim = 480;
            fb.rgba = true;
        }
        Ok(Self {
            core,
            console_out: Rc::new(RefCell::new(Vec::new())),
            root,
            #[cfg(feature = "sfx")]
            sfx,
            #[cfg(feature = "midi-desktop")]
            midi_io,
        })
    }

    pub fn install(&self, vm: &mut VirtualMachine) {
        self.core.install(vm);
        #[cfg(feature = "sfx")]
        self.sfx.install(vm);
        #[cfg(all(feature = "gfx", feature = "fs"))]
        crate::gfx::register_io_natives(vm, Rc::clone(&self.gfx.fb), self.root.clone());
        #[cfg(feature = "midi-desktop")]
        if let Some(m) = &self.midi_io { m.install(vm); }
    }

    #[cfg(feature = "compiler")]
    pub fn register_host_fns(&self, compiler: &mut abrase::compiler::Compiler) -> Result<(), String> {
        use abrase::ast::EffectItem;
        use abrase::ty::Type as T;
        use posara_core::abi::{Cap, Eff, Ty, NATIVES};

        fn enabled(cap: Cap) -> bool {
            match cap {
                Cap::Core => true,
                Cap::Gfx | Cap::Input => cfg!(feature = "gfx"),
                Cap::GfxFs => cfg!(all(feature = "gfx", feature = "fs")),
                Cap::Sfx => cfg!(feature = "sfx"),
                Cap::SfxFs => cfg!(all(feature = "sfx", feature = "fs")),
                Cap::Synth => cfg!(feature = "synth"),
                Cap::Fs => cfg!(feature = "fs"),
                Cap::Midi => cfg!(feature = "midi"),
                Cap::Net => cfg!(feature = "net"),
                Cap::Imu => cfg!(feature = "imu"),
            }
        }
        fn ty(t: Ty) -> T {
            let bytes = || T::Named("Bytes".into());
            let arr_int = || T::Generic { name: "Array".into(), args: vec![T::Int] };
            match t {
                Ty::Int => T::Int,
                Ty::Float => T::Float,
                Ty::Bool => T::Bool,
                Ty::Str => T::String,
                Ty::Unit => T::Unit,
                Ty::Bytes => bytes(),
                Ty::RefBytes => T::Reference { is_mut: false, inner: Box::new(bytes()) },
                Ty::ArrInt => arr_int(),
                Ty::RefArrInt => T::Reference { is_mut: false, inner: Box::new(arr_int()) },
            }
        }
        fn eff(e: Eff) -> Vec<EffectItem> {
            let name = match e {
                Eff::Pure => return vec![],
                Eff::Graphics => "Graphics",
                Eff::IO => "IO",
                Eff::IOExnString => return vec![
                    EffectItem { name: vec!["IO".into()], arg: None },
                    EffectItem { name: vec!["exn".into()], arg: Some(Box::new(abrase::ast::Type::Named("String".into()))) },
                ],
                Eff::Network => "net",
                Eff::Nondet => "nondet",
            };
            vec![EffectItem { name: vec![name.into()], arg: None }]
        }

        for s in NATIVES.iter().filter(|s| enabled(s.cap)) {
            compiler.register_host_fn(s.name, s.params.iter().map(|&t| ty(t)).collect(), ty(s.ret), eff(s.eff))?;
        }
        Ok(())
    }
}
