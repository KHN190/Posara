pub use myriad::word32 as runtime;
pub use posara_core::word32 as profile_core;
#[path="host32_backend.rs"]
pub mod backend;
#[path="host.rs"]
pub mod host;
#[cfg(feature="gfx")]
#[path="plugins/gfx/mod.rs"]
pub mod gfx_glue;
#[cfg(feature="sfx")]
#[path="plugins/sfx/plugin.rs"]
pub mod sfx_plugin;
#[cfg(feature="midi-desktop")]
#[path="plugins/midi/mod.rs"]
pub mod midi_plugin;

impl host::Host {
    pub fn from_host64(host64: &crate::Host) -> Result<Self, String> {
        let clock = std::rc::Rc::new(backend::ClockAdapter(std::rc::Rc::clone(&host64.clock_src)));
        #[cfg(feature = "fs")]
        let storage = std::rc::Rc::new(backend::StorageAdapter(std::rc::Rc::clone(&host64.fs.storage)));
        #[cfg(not(feature = "fs"))]
        let storage = std::rc::Rc::new(profile_core::fs::MemStorage::new());

        #[cfg(feature = "gfx")]
        let display = Some(Box::new(backend::PresenterAdapter::Framebuffer(std::rc::Rc::clone(&host64.gfx.fb))) as Box<dyn profile_core::drivers::Presenter>);

        #[cfg(feature = "sfx")]
        let cmds = std::sync::Arc::clone(&host64.sfx.audio.cmds);

        let core = profile_core::host::Host::new(profile_core::host::Drivers {
            clock,
            #[cfg(feature = "gfx")]
            display,
            #[cfg(feature = "sfx")]
            cmds,
            storage,
            #[cfg(feature = "midi")]
            midi: std::rc::Rc::clone(&host64.core.midi.port),
            #[cfg(feature = "net")]
            sockets: Box::new(backend::SocketsAdapter::shared(std::rc::Rc::clone(&host64.net.net))),
            #[cfg(feature = "imu")]
            imu: std::rc::Rc::clone(&host64.imu.imu),
        });

        #[cfg(feature = "sfx")]
        let sfx = sfx_plugin::SfxPlugin::from_audio(host64.sfx.audio.share(), std::rc::Rc::clone(&host64.sfx.recorder), host64.sfx.root.clone());

        Ok(Self {
            core,
            console_out: std::rc::Rc::clone(&host64.console_out),
            root: host64.root.clone(),
            #[cfg(feature = "sfx")]
            sfx,
            #[cfg(feature = "midi-desktop")]
            midi_io: host64.midi_io.as_ref().map(|m| m.share32()),
        })
    }
}
