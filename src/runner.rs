use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use myriad::VirtualMachine;
use myriad::devices::{Console, SystemDevice, CONSOLE_ID, SYSTEM_ID};
use polka::{Module, Value};

use crate::Host;

use std::io::Write;
struct StdoutConsole;
impl Console for StdoutConsole {
    fn read_byte(&mut self) -> Result<Option<u8>, String> { Ok(None) }
    fn write_stdout(&mut self, b: u8) -> Result<(), String> {
        std::io::stdout().write_all(&[b]).map_err(|e| e.to_string())
    }
    fn write_stderr(&mut self, b: u8) -> Result<(), String> {
        std::io::stderr().write_all(&[b]).map_err(|e| e.to_string())
    }
    fn flush(&mut self) -> Result<(), String> {
        std::io::stdout().flush().map_err(|e| e.to_string())
    }
}

const FRAME: Duration = Duration::from_micros(16_667);

// Per-frame ops budget. Reaching this prints a rate-limited warning. Hard
// mid-frame skip would need a mutable step_cap setter in myriad.
const OPS_BUDGET: u64 = 500_000;

fn mtime(p: &Path) -> Option<SystemTime> {
    std::fs::metadata(p).and_then(|m| m.modified()).ok()
}

// Without gfx there is no window to close; the loop runs until halt / deadline.
#[cfg(feature = "gfx")]
fn alive(host: &Host) -> bool { host.gfx.fb.borrow().alive }
#[cfg(not(feature = "gfx"))]
fn alive(_host: &Host) -> bool { true }

pub use crate::debug::DebugCfg;

fn make_vm(host: &Host, static_names: Vec<String>, fn_names: Vec<String>, dbg: DebugCfg) -> VirtualMachine {
    let mut vm = VirtualMachine::new()
        .with_static_names(static_names)
        .with_fn_names(fn_names)
        .with_profile(std::env::var("PROFILE").as_deref() == Ok("1"));
    if dbg.needs_sink() {
        vm = vm.with_debug_sink(crate::debug::sink(dbg.trace, dbg.handlers));
    }
    vm.install_device(SYSTEM_ID, Box::new(SystemDevice::new()));
    let console: Box<dyn Console> = Box::new(StdoutConsole);
    vm.install_device(CONSOLE_ID, Box::new(console));
    host.install(&mut vm);
    vm
}

pub struct LoadResult {
    pub module: Module,
    pub static_names: Vec<String>,
    pub fn_names: Vec<String>,
    pub warnings: Vec<crate::lint::PosaraLint>,
}

pub fn load_module(path: &Path, host: &Host) -> Result<LoadResult, String> {
    match path.extension().and_then(|s| s.to_str()) {
        Some("pk")  => load_pk(path).map(|m| LoadResult {
            module: m, static_names: vec![], fn_names: vec![], warnings: vec![],
        }),
        Some("abe") => compile_abe(path, host),
        _ => Err(format!("unknown extension (expected .abe or .pk): {}", path.display())),
    }
}

fn is_cart(module: &Module) -> bool {
    module.functions.iter().any(|c| matches!(c, polka::Chunk::Native(n) if n.name == "__frame_present"))
}

pub fn load_pk(path: &Path) -> Result<Module, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    polka::cartridge::read_pk(&bytes).map_err(|e| format!("read_pk: {e:?}"))
}

fn module_root(entry: &Path) -> Option<PathBuf> {
    let mut dir = entry.parent();
    while let Some(d) = dir {
        if d.join("posara.toml").is_file() { return Some(d.to_path_buf()); }
        dir = d.parent();
    }
    None
}

// Default host root for a cart: the posara.toml project dir if one is found
// above the entry file, otherwise the entry's own directory. Keeps fs paths
// ("assets/...") stable no matter how deep the cart sits.
pub fn default_root(entry: &Path) -> PathBuf {
    module_root(entry).unwrap_or_else(|| {
        entry.parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
    })
}

#[cfg(feature = "compiler")]
pub fn compile_abe(path: &Path, host: &Host) -> Result<LoadResult, String> {
    use abrase::compiler::Compiler;
    use abrase::loader;
    use crate::lint::{from_abrase_lint, lint_module};

    let mroot = module_root(path);
    match &mroot {
        Some(r) => eprintln!("• module root: {} (posara.toml)", r.display()),
        None => eprintln!("• module root: {} (no posara.toml found, using entry dir)",
            path.parent().unwrap_or_else(|| Path::new(".")).display()),
    }
    let program = loader::load_program_with_root(path, mroot.as_deref()).map_err(|e| e.to_string())?;
    let mut compiler = Compiler::new().with_source(program.entry_source.clone());
    host.register_host_fns(&mut compiler)?;
    let module = compiler.compile_module(&program.decls)
        .map_err(|errs| program.render_errors(&errs))?;

    let mut warnings: Vec<crate::lint::PosaraLint> = compiler.warnings.iter()
        .map(from_abrase_lint)
        .collect();
    warnings.extend(lint_module(&module));

    Ok(LoadResult {
        static_names: compiler.static_names_by_offset(),
        fn_names: compiler.fn_names(),
        module,
        warnings,
    })
}

#[cfg(not(feature = "compiler"))]
pub fn compile_abe(_path: &Path, _host: &Host) -> Result<LoadResult, String> {
    Err("posara built without `compiler` feature; only .pk supported".into())
}

#[cfg(feature = "compiler")]
pub fn build_crate(entry: &Path, host: &Host, out: &Path) -> Result<PathBuf, String> {
    let module = compile_abe(entry, host)?.module;
    let lib_src = polka_rustc::transpile_module_lib(&module)
        .map_err(|e| format!("transpile: {e}"))?;

    let stem = entry.file_stem().and_then(|s| s.to_str()).unwrap_or("cart");
    let root = std::fs::canonicalize(default_root(entry)).unwrap_or_else(|_| default_root(entry));
    let entry_abs = std::fs::canonicalize(entry).unwrap_or_else(|_| entry.to_path_buf());
    let entry = entry_abs.as_path();
    let posara = env!("CARGO_MANIFEST_DIR");

    std::fs::create_dir_all(out.join("src")).map_err(|e| format!("mkdir: {e}"))?;
    std::fs::write(out.join("src/lib.rs"), lib_src).map_err(|e| format!("write lib.rs: {e}"))?;

    let main_src = format!(
        "use std::path::{{Path, PathBuf}};\n\
         use std::process::ExitCode;\n\
         fn main() -> ExitCode {{\n\
         \x20   let root = std::env::args().nth(1).map(PathBuf::from).unwrap_or_else(|| PathBuf::from({root:?}));\n\
         \x20   let host = match posara::Host::new_cart(root, false, false, Path::new({entry:?})) {{\n\
         \x20       Ok(h) => h, Err(e) => {{ eprintln!(\"host init: {{e}}\"); return ExitCode::from(1); }}\n\
         \x20   }};\n\
         \x20   match posara::runner::run_aot(aotcart::PK, aotcart::register_aot, &host) {{\n\
         \x20       Ok(code) => ExitCode::from(code as u8),\n\
         \x20       Err(e) => {{ eprintln!(\"{{e}}\"); ExitCode::from(1) }}\n\
         \x20   }}\n\
         }}\n",
        root = root.to_string_lossy(),
        entry = entry.to_string_lossy(),
    );
    std::fs::write(out.join("src/main.rs"), main_src).map_err(|e| format!("write main.rs: {e}"))?;

    let cargo = format!(
        "[package]\n\
         name = \"{stem}-build-rs\"\n\
         version = \"0.0.0\"\n\
         edition = \"2024\"\n\n\
         [lib]\n\
         name = \"aotcart\"\n\
         path = \"src/lib.rs\"\n\n\
         [[bin]]\n\
         name = \"{stem}\"\n\
         path = \"src/main.rs\"\n\n\
         [dependencies]\n\
         posara = {{ path = {posara:?}, default-features = false, features = [\"gfx\", \"sfx\", \"synth\", \"fs\", \"midi\"] }}\n\
         myriad-rs = {{ git = \"https://github.com/KHN190/Abrase\", branch = \"dev\" }}\n\
         polka-rs = {{ git = \"https://github.com/KHN190/Abrase\", branch = \"dev\" }}\n",
    );
    std::fs::write(out.join("Cargo.toml"), cargo).map_err(|e| format!("write Cargo.toml: {e}"))?;

    Ok(out.to_path_buf())
}

#[cfg(not(feature = "compiler"))]
pub fn build_crate(_entry: &Path, _host: &Host, _out: &Path) -> Result<PathBuf, String> {
    Err("posara built without `compiler` feature".into())
}

// Drives a loaded module frame by frame, holding the live VM state. Tests use
// it to inject input between frames; run_until_ms / run_module wrap it.
pub struct Stepper<'a> {
    vm: VirtualMachine,
    loaded: myriad::loader::LoadedModule,
    host: &'a Host,
    cart: bool,
    has_update: bool,
    done: bool,
}

impl<'a> Stepper<'a> {
    pub fn start(module: Module, static_names: Vec<String>, host: &'a Host) -> Result<Self, String> {
        Self::start_named(module, static_names, Vec::new(), host)
    }

    pub fn start_named(module: Module, static_names: Vec<String>, fn_names: Vec<String>, host: &'a Host) -> Result<Self, String> {
        Self::start_inner(module, static_names, fn_names, host, |_| {})
    }

    pub fn start_aot<F: FnOnce(&mut VirtualMachine)>(module: Module, host: &'a Host, register_aot: F) -> Result<Self, String> {
        Self::start_inner(module, Vec::new(), Vec::new(), host, register_aot)
    }

    fn start_inner<F: FnOnce(&mut VirtualMachine)>(module: Module, static_names: Vec<String>, fn_names: Vec<String>, host: &'a Host, register_aot: F) -> Result<Self, String> {
        let loaded = myriad::loader::load(module)?;
        let mut vm = make_vm(host, static_names, fn_names, DebugCfg::default());
        register_aot(&mut vm);
        let cart = is_cart(&loaded.module);
        if cart {
            vm.run_to_yield(&loaded.module)?;
        } else if loaded.module.exports.iter().any(|e| e.name == "start") {
            vm.call_export(&loaded.module, "start", &[])?;
        }
        let has_update = loaded.module.exports.iter().any(|e| e.name == "update");
        Ok(Self { vm, loaded, host, cart, has_update, done: false })
    }

    pub fn is_frame_loop(&self) -> bool { self.cart || self.has_update }
    pub fn steps(&self) -> u64 { self.vm.steps() }
    pub fn exit_code(&self) -> i64 { self.vm.exit_code().unwrap_or(0) }
    pub fn print_profile(&self) {
        if std::env::var("PROFILE").as_deref() == Ok("1") { eprint!("{}", self.vm.profile_report()); }
    }

    // Run one frame; returns false once the program is finished.
    pub fn frame(&mut self) -> Result<bool, String> {
        if self.done { return Ok(false); }
        if self.cart {
            if !self.vm.resume(&self.loaded.module, Value::from_int(0))? { self.done = true; }
        } else if self.has_update {
            self.vm.call_export(&self.loaded.module, "update", &[])?;
            if self.vm.halted() { self.done = true; }
        } else {
            let _: Value = self.vm.run_module(&self.loaded.module)?;
            self.done = true;
        }
        #[cfg(feature = "sfx")]
        self.host.sfx.drain_recorder();
        Ok(!self.done)
    }
}

pub fn run_until_frame(module: Module, static_names: Vec<String>, fn_names: Vec<String>, host: &Host, n: u64) -> Result<(), String> {
    let mut step = Stepper::start_named(module, static_names, fn_names, host)?;
    for _ in 0..n {
        if !step.frame()? { break; }
    }
    step.print_profile();
    #[cfg(feature = "gfx")]
    {
        let fb = host.gfx.fb.borrow();
        if fb.w > 0 && fb.commits == 0 {
            return Err("update ran but never committed screen (device_in(0x2001,1)); window would stay blank".into());
        }
    }
    Ok(())
}

// Offline: virtual clock, no sleep — runs to the deadline as fast as possible.
// Used by `dump --at-ms` so a time-point grab lands instantly, not after T ms.
pub fn run_until_ms(module: Module, static_names: Vec<String>, fn_names: Vec<String>, host: &Host, deadline_ms: u64) -> Result<(), String> {
    host.clock.set(Some(0));
    let mut step = Stepper::start_named(module, static_names, fn_names, host)?;
    if step.is_frame_loop() {
        let mut vms = 0.0f64;
        while (vms as u64) < deadline_ms {
            if !alive(host) { break; }
            host.clock.set(Some(vms as u64));
            if !step.frame()? { break; }
            vms += 1000.0 / 60.0;
        }
    } else {
        step.frame()?;
    }
    host.clock.set(None);
    #[cfg(feature = "gfx")]
    {
        let fb = host.gfx.fb.borrow();
        if fb.w > 0 && fb.commits == 0 {
            return Err("update ran but never committed screen (device_in(0x2001,1)); window would stay blank".into());
        }
    }
    Ok(())
}

// Offline render: drive a virtual clock, mix the synth/sfx commands the cart
// emits straight to a WAV — silent, deterministic, faster than real time. With
// a frames dir it also saves PNGs on the fps grid (gfx only). No audio device.
#[cfg(feature = "sfx")]
pub fn run_render(
    module: Module, static_names: Vec<String>, fn_names: Vec<String>, host: &Host,
    dur_ms: u64, out: &Path, frames: Option<PathBuf>, fps: u32, from_ms: u64,
) -> Result<(), String> {
    let sr = host.sfx.audio.sample_rate;
    let ch = host.sfx.audio.channels.max(1) as usize;
    let mut mixer = crate::sfx::Mixer::new(sr, ch, host.sfx.audio.meter.clone());
    let spf = (sr / 60).max(1) as usize;       // samples per video frame, per channel
    let mut frame_buf = vec![0.0f32; spf * ch];
    let mut samples: Vec<f32> = Vec::with_capacity((dur_ms as usize) * sr as usize / 1000 * ch);

    host.clock.set(Some(0));
    let mut step = Stepper::start_named(module, static_names, fn_names, host)?;
    if !step.is_frame_loop() {
        host.clock.set(None);
        return Err("render needs a frame-loop cart (update() or @cart)".into());
    }
    if let Some(dir) = &frames {
        std::fs::create_dir_all(dir).map_err(|e| format!("frames dir: {e}"))?;
    }
    let interval = 1000.0 / fps as f64;
    let mut vms = 0.0f64;
    let mut shot: u64 = 0;
    while (vms as u64) < dur_ms {
        host.clock.set(Some(vms as u64));
        if !step.frame()? { break; }
        while let Some(cmd) = host.sfx.audio.cmds.try_pop() { mixer.apply(cmd); }
        mixer.mix(&mut frame_buf);
        samples.extend_from_slice(&frame_buf);
        #[cfg(feature = "gfx")]
        if let Some(dir) = &frames {
            if vms >= from_ms as f64 + shot as f64 * interval {
                let fb = host.gfx.fb.borrow();
                fb.save_region_png(0, 0, fb.w as i64, fb.h as i64, &dir.join(format!("{shot:04}.png")))?;
                shot += 1;
            }
        }
        vms += 1000.0 / 60.0;
    }
    host.clock.set(None);
    let _ = (from_ms, interval, &mut shot, fps);
    crate::sfx::write_wav_f32(out, sr, ch as u16, &samples)
}

// Real-time run that saves a PNG every 1000/fps ms (starting at from_ms) while
// the recorder (if armed) captures audio — one run, synced tracks.
#[cfg(feature = "gfx")]
pub struct CaptureCfg {
    pub dir: PathBuf,
    pub fps: u32,
    pub from_ms: u64,
}

#[cfg(feature = "gfx")]
pub fn run_capture(module: Module, static_names: Vec<String>, fn_names: Vec<String>, host: &Host, deadline_ms: u64, cap: CaptureCfg) -> Result<u64, String> {
    std::fs::create_dir_all(&cap.dir).map_err(|e| format!("capture dir: {e}"))?;
    let mut step = Stepper::start_named(module, static_names, fn_names, host)?;
    if !step.is_frame_loop() {
        return Err("capture needs a frame-loop cart (update() or @cart)".into());
    }
    let interval_ms = 1000.0 / cap.fps as f64;
    let mut shot: u64 = 0;
    loop {
        if !alive(host) { break; }
        let elapsed = host.start.elapsed().as_millis() as u64;
        if elapsed >= deadline_ms { break; }
        let t0 = Instant::now();
        if !step.frame()? { break; }
        let due = cap.from_ms as f64 + shot as f64 * interval_ms;
        if (elapsed as f64) >= due {
            let fb = host.gfx.fb.borrow();
            let path = cap.dir.join(format!("{shot:04}.png"));
            fb.save_region_png(0, 0, fb.w as i64, fb.h as i64, &path)?;
            shot += 1;
        }
        let elapsed = t0.elapsed();
        if elapsed < FRAME { std::thread::sleep(FRAME - elapsed); }
    }
    Ok(shot)
}

// Run every `pub fn test_*()` export, each in a fresh VM for isolation. Returns
// true when all passed. assert/assert_eq failures surface as the test's Err.
#[cfg(feature = "test")]
pub fn run_tests(module: Module, static_names: Vec<String>, fn_names: Vec<String>, host: &Host) -> Result<bool, String> {
    let loaded = myriad::loader::load(module)?;
    let names: Vec<String> = loaded.module.exports.iter()
        .map(|e| e.name.clone())
        .filter(|n| n.starts_with("test_"))
        .collect();
    if names.is_empty() { return Err("no `test_*` exports found".into()); }
    let mut failed = 0;
    for name in &names {
        let mut vm = make_vm(host, static_names.clone(), fn_names.clone(), DebugCfg::default());
        match vm.call_export(&loaded.module, name, &[]) {
            Ok(_) => eprintln!("ok   {name}"),
            Err(e) => { eprintln!("FAIL {name}: {e}"); failed += 1; }
        }
    }
    eprintln!("{} passed, {failed} failed", names.len() - failed);
    Ok(failed == 0)
}

pub fn run_aot<F: FnOnce(&mut VirtualMachine)>(pk: &[u8], register_aot: F, host: &Host) -> Result<i64, String> {
    let module = polka::cartridge::read_pk(pk).map_err(|e| format!("read_pk: {e:?}"))?;
    let mut step = Stepper::start_aot(module, host, register_aot)?;
    loop {
        if !alive(host) { break; }
        let t0 = Instant::now();
        if !step.frame()? { break; }
        let elapsed = t0.elapsed();
        if elapsed < FRAME { std::thread::sleep(FRAME - elapsed); }
    }
    step.print_profile();
    Ok(step.exit_code())
}

struct OverBudgetWarn { hits: u32, last_log: Instant }
impl OverBudgetWarn {
    fn new() -> Self { Self { hits: 0, last_log: Instant::now() - Duration::from_secs(10) } }
    fn check(&mut self, delta: u64) {
        if delta <= OPS_BUDGET { return; }
        self.hits += 1;
        if self.last_log.elapsed() >= Duration::from_secs(2) {
            eprintln!("• ops/frame {}k > budget {}k ({} hits since last warn)", delta/1000, OPS_BUDGET/1000, self.hits);
            self.hits = 0;
            self.last_log = Instant::now();
        }
    }
}

pub fn run_module(module: Module, static_names: Vec<String>, fn_names: Vec<String>, host: &Host, reload: Option<PathBuf>, profile: bool, dbg: DebugCfg) -> Result<i64, String> {
    let mut loaded = myriad::loader::load(module)?;
    let mut vm = make_vm(host, static_names, fn_names, dbg);

    let mut cart = is_cart(&loaded.module);
    let has_update = loaded.module.exports.iter().any(|e| e.name == "update");
    if !cart && !has_update {
        let v: Value = vm.run_module(&loaded.module)?;   // one-shot self-contained
        if dbg.leak { eprint!("{}", vm.live_slots_report()); }
        return Ok(v.as_int());
    }

    let start_of = |l: &myriad::loader::LoadedModule| l.module.exports.iter().any(|e| e.name == "start");
    if cart {
        vm.run_to_yield(&loaded.module)?;   // run main to the first frame.present()
    } else if start_of(&loaded) {
        vm.call_export(&loaded.module, "start", &[])?;
    }

    #[cfg(feature = "gfx")]
    let mut prof = if profile { crate::profile::Profiler::new() } else { None };
    #[cfg(not(feature = "gfx"))]
    let _ = profile;
    #[cfg(feature = "gfx")]
    let mut last_steps: u64 = 0;
    let mut prev = Instant::now();

    let mut last = reload.as_deref().and_then(mtime);
    if let Some(p) = reload.as_deref() {
        eprintln!("• hot reload armed: {}", p.display());
    }
    let mut frame: u64 = 0;
    let mut overbudget = OverBudgetWarn::new();
    let exit_code = loop {
        if !alive(host) { break 0i64; }
        let t0 = Instant::now();
        let frame_dt = t0 - prev;
        prev = t0;
        let s_pre = vm.steps();
        if cart {
            let still = vm.resume(&loaded.module, Value::from_int(0))?;   // run to next present()
            overbudget.check(vm.steps() - s_pre);
            if !still { break vm.exit_code().unwrap_or(0); }              // main returned / halted
        } else {
            vm.call_export(&loaded.module, "update", &[])?;
            overbudget.check(vm.steps() - s_pre);
            if vm.halted() { break vm.exit_code().unwrap_or(0); }
        }
        #[cfg(feature = "sfx")]
        host.sfx.drain_recorder();
        #[cfg(feature = "gfx")]
        {
            let work = t0.elapsed();
            if prof.as_ref().is_some_and(|p| !p.is_open()) { prof = None; }
            if let Some(p) = prof.as_mut() {
                let steps = vm.steps();
                let ops = steps.saturating_sub(last_steps);
                last_steps = steps;
                p.sample(ops, vm.heap_ref().bytes_used(), vm.heap_live_count(), work, frame_dt);
                #[cfg(feature = "sfx")]
                {
                    let m = host.sfx.audio.meter.snapshot();
                    p.set_audio(m.notes, m.out_peak, m.ch_voices, m.ch_peak);
                }
                p.draw();
            }
        }
        #[cfg(not(feature = "gfx"))]
        let _ = (prev, frame_dt);

        frame += 1;
        if let Some(p) = reload.as_deref() {
            if frame % 10 == 0 {
                let now = mtime(p);
                if now != last {
                    last = now;
                    match load_module(p, host).and_then(|r| myriad::loader::load(r.module).map(|l| (l, r.static_names, r.fn_names))) {
                        Ok((newl, new_static, new_fns)) => {
                            loaded = newl;
                            vm = make_vm(host, new_static, new_fns, dbg);
                            #[cfg(feature = "gfx")]
                            { last_steps = 0; }
                            cart = is_cart(&loaded.module);
                            let init = if cart {
                                vm.run_to_yield(&loaded.module)
                            } else if start_of(&loaded) {
                                vm.call_export(&loaded.module, "start", &[]).map(|_| ())
                            } else { Ok(()) };
                            if let Err(e) = init { eprintln!("reload init error: {e}"); }
                            eprintln!("• reloaded {}", p.display());
                        }
                        Err(e) => eprintln!("• reload failed (keeping old):\n{e}"),
                    }
                }
            }
        }

        let elapsed = t0.elapsed();
        if elapsed < FRAME { std::thread::sleep(FRAME - elapsed); }
    };
    if dbg.leak { eprint!("{}", vm.live_slots_report()); }
    Ok(exit_code)
}
