pub mod env;
mod fx;
mod lfo;
pub mod mixer;
pub mod output;
pub mod record;
mod sample;
mod seq;
pub mod spsc;
#[cfg(feature = "synth")]
pub mod synth;
mod voice;

pub use mixer::Mixer;
pub use output::{Audio, AudioMeter, AudioSnapshot, CmdProd, Meter, SampleRing};
pub use record::Recorder;

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
#[cfg(feature = "fs")]
use std::sync::atomic::AtomicBool;

use myriad::{NativeCtx, Value, VirtualMachine};
#[cfg(feature = "fs")]
use myriad::read_string;

use mixer::Cmd;
use output::push;

use crate::plugin::Plugin;

pub type RecorderSlot = Rc<RefCell<Option<Recorder>>>;

pub struct SfxPlugin {
    pub audio: Audio,
    pub recorder: RecorderSlot,
    root: PathBuf,
}

impl SfxPlugin {
    pub fn new(root: PathBuf) -> Result<Self, String> {
        Ok(Self {
            audio: Audio::new()?,
            recorder: Rc::new(RefCell::new(None)),
            root,
        })
    }

    pub fn drain_recorder(&self) {
        if let Some(r) = self.recorder.borrow_mut().as_mut() {
            if let Err(e) = r.drain() {
                eprintln!("recorder drain: {e}");
            }
        }
    }
}

impl Drop for SfxPlugin {
    fn drop(&mut self) {
        if let Some(r) = self.recorder.borrow_mut().as_mut() {
            let _ = r.stop();
        }
    }
}

impl Plugin for SfxPlugin {
    fn install(&self, vm: &mut VirtualMachine) {
        register_natives(vm, Arc::clone(&self.audio.cmds));
        #[cfg(feature = "fs")]
        register_record_natives(
            vm,
            Arc::clone(&self.audio.rec_ring),
            Arc::clone(&self.audio.rec_on),
            self.audio.sample_rate,
            self.audio.channels,
            self.root.clone(),
            Rc::clone(&self.recorder),
        );
    }

    #[cfg(feature = "compiler")]
    fn register_fns(&self, compiler: &mut abrase::compiler::Compiler) -> Result<(), String> {
        use abrase::ast::EffectItem;
        let io_eff = || vec![EffectItem { name: vec!["IO".into()], arg: None }];
        for (name, params, ret) in host_fn_decls() {
            compiler.register_host_fn(name, params, ret, io_eff())?;
        }
        Ok(())
    }
}

fn arg(args: &[Value], i: usize) -> i64 {
    args.get(i).copied().unwrap_or(Value::ZERO).as_int()
}

fn ret_unit() -> Result<(Value, bool), String> { Ok((Value::UNIT, false)) }

// Every sfx native pushes one Cmd onto the lock-free ring; the audio thread
// drains it. Ring full → command dropped (`let _`), which only happens under
// absurd command spam — control rate is tiny vs the 1024-slot ring.
pub fn register_natives(vm: &mut VirtualMachine, cmds: CmdProd) {
    macro_rules! native {
        ($name:literal, |$a:ident| $cmd:expr) => {{
            let p = Arc::clone(&cmds);
            vm.register_native($name, Rc::new(move |_ctx: &mut NativeCtx, $a: &[Value]| {
                push(&p, $cmd);
                ret_unit()
            }));
        }};
    }

    // legacy fire-and-forget sugar
    native!("sfx_tone", |a| Cmd::Tone(arg(a, 0) as f32, arg(a, 1).max(0) as u32,
        (arg(a, 2).clamp(0, 100) as f32) / 100.0, arg(a, 3).max(0) as u32));
    native!("sfx_noise", |a| Cmd::Noise(arg(a, 0).max(0) as u32,
        (arg(a, 1).clamp(0, 100) as f32) / 100.0, arg(a, 2).max(0) as u32));
    native!("sfx_wave", |a| Cmd::Wave(arg(a, 0).clamp(0, 4) as u8, arg(a, 1) as f32,
        arg(a, 2).max(0) as u32, (arg(a, 3).clamp(0, 100) as f32) / 100.0, arg(a, 4).max(0) as u32));

    // synth patch
    native!("sfx_inst", |a| Cmd::Inst(arg(a, 0).max(0) as usize, arg(a, 1).clamp(0, 4) as u8,
        arg(a, 2).max(0) as u32, arg(a, 3).max(0) as u32,
        (arg(a, 4).clamp(0, 100) as f32) / 100.0, arg(a, 5).max(0) as u32));
    native!("sfx_pan", |a| Cmd::Pan(arg(a, 0).max(0) as usize,
        (arg(a, 1).clamp(0, 100) as f32) / 100.0, (arg(a, 2).clamp(0, 100) as f32) / 100.0));
    native!("sfx_fx", |a| Cmd::Fx(arg(a, 0).max(0) as usize, arg(a, 1).clamp(0, 5) as u8,
        (arg(a, 2).clamp(0, 100) as f32) / 100.0, arg(a, 3).max(0) as f32));
    native!("sfx_lfo", |a| Cmd::Lfo(arg(a, 0).max(0) as usize, arg(a, 1).clamp(0, 2) as u8,
        arg(a, 2).clamp(0, 3) as u8, arg(a, 3).max(0) as f32 / 100.0,
        (arg(a, 4).clamp(0, 100) as f32) / 100.0));

    // trigger
    native!("sfx_play", |a| Cmd::Play(arg(a, 0).max(0) as usize, arg(a, 1) as f32,
        (arg(a, 2).clamp(0, 100) as f32) / 100.0, arg(a, 3).max(0) as u32));
    native!("sfx_playm", |a| Cmd::PlayMidi(arg(a, 0).max(0) as usize, arg(a, 1),
        (arg(a, 2).clamp(0, 100) as f32) / 100.0, arg(a, 3).max(0) as u32));
    native!("sfx_off", |a| Cmd::Off(arg(a, 0).max(0) as usize));

    // sequencer
    let p = Arc::clone(&cmds);
    vm.register_native("sfx_seq", Rc::new(move |ctx: &mut NativeCtx, a: &[Value]| {
        let pattern = a.first().copied().unwrap_or(Value::NONE);
        let ms_per_tick = arg(a, 1).max(1) as u32;
        if pattern.is_handle_none() { return ret_unit(); }
        let (slot, gen_) = pattern.as_handle();
        let cells = ctx.heap.cell_data(slot, gen_)?;
        let events: Vec<seq::Event> = cells.iter().map(|&w| seq::unpack(w as i64)).collect();
        push(&p, Cmd::Seq(events, ms_per_tick));
        ret_unit()
    }));
    // sfx_track: like sfx_seq but the array is a packed byte stream (each elem's
    // low 8 bits = one byte), 8 LE bytes per i64 event — the layout fs_read gives
    // from a .trk asset. All-zero words (padding) skipped.
    let p = Arc::clone(&cmds);
    vm.register_native("sfx_track", Rc::new(move |ctx: &mut NativeCtx, a: &[Value]| {
        let bytes = a.first().copied().unwrap_or(Value::NONE);
        let ms_per_tick = arg(a, 1).max(1) as u32;
        if bytes.is_handle_none() { return ret_unit(); }
        let (slot, gen_) = bytes.as_handle();
        let cells = ctx.heap.cell_data(slot, gen_)?;
        let mut events: Vec<seq::Event> = Vec::with_capacity(cells.len() / 8);
        for chunk in cells.chunks(8) {
            let mut word: u64 = 0;
            for (b, &c) in chunk.iter().enumerate() {
                word |= (c & 0xFF) << (8 * b);
            }
            if word != 0 { events.push(seq::unpack(word as i64)); }
        }
        push(&p, Cmd::Seq(events, ms_per_tick));
        ret_unit()
    }));
    native!("sfx_seqstop", |_a| Cmd::SeqStop);

    // sfx_sample: play a 1-bit delta-sigma stream (fs_read bytes, low 8 bits =
    // one byte, MSB-first) at `rate` Hz — the mp32sample playback path.
    let p = Arc::clone(&cmds);
    vm.register_native("sfx_sample", Rc::new(move |ctx: &mut NativeCtx, a: &[Value]| {
        let buf = a.first().copied().unwrap_or(Value::NONE);
        let rate = arg(a, 1).max(1) as f32;
        let vol = (arg(a, 2).clamp(0, 100) as f32) / 100.0;
        if buf.is_handle_none() { return ret_unit(); }
        let (slot, gen_) = buf.as_handle();
        let cells = ctx.heap.cell_data(slot, gen_)?;
        let bytes: Vec<u8> = cells.iter().map(|&c| (c & 0xFF) as u8).collect();
        push(&p, Cmd::Sample(bytes, rate, vol));
        ret_unit()
    }));
    native!("sfx_samplestop", |_a| Cmd::SampleStop);
}

#[cfg(feature = "fs")]
pub fn register_record_natives(
    vm: &mut VirtualMachine,
    ring: SampleRing,
    enabled: Arc<AtomicBool>,
    sample_rate: u32,
    channels: u16,
    root: PathBuf,
    rec: RecorderSlot,
) {
    let r = root.clone();
    let rs = Rc::clone(&rec);
    let rg = Arc::clone(&ring);
    let en = Arc::clone(&enabled);
    vm.register_native("sfx_record_start", Rc::new(move |ctx: &mut NativeCtx, a: &[Value]| {
        let Some(rel) = a.first().copied().and_then(|v| read_string(ctx.heap, v)) else {
            return Ok((Value::from_int(-1), false));
        };
        let Some(path) = crate::fs::resolve(&r, &rel) else {
            return Ok((Value::from_int(-1), false));
        };
        if rs.borrow().is_some() {
            return Ok((Value::from_int(-1), false));
        }
        match Recorder::start(&path, sample_rate, channels, Arc::clone(&rg), Arc::clone(&en)) {
            Ok(r) => { *rs.borrow_mut() = Some(r); Ok((Value::from_int(0), false)) }
            Err(e) => { eprintln!("sfx_record_start: {e}"); Ok((Value::from_int(-1), false)) }
        }
    }));
    let rs = Rc::clone(&rec);
    vm.register_native("sfx_record_stop", Rc::new(move |_ctx: &mut NativeCtx, _a: &[Value]| {
        let mut slot = rs.borrow_mut();
        let Some(r) = slot.as_mut() else {
            return Ok((Value::from_int(-1), false));
        };
        let result = match r.stop() {
            Ok(()) => 0,
            Err(e) => { eprintln!("sfx_record_stop: {e}"); -1 }
        };
        *slot = None;
        Ok((Value::from_int(result), false))
    }));
}

#[cfg(feature = "compiler")]
pub fn host_fn_decls() -> Vec<(&'static str, Vec<abrase::ty::Type>, abrase::ty::Type)> {
    use abrase::ty::Type as T;
    let arr_int = || T::Generic { name: "Array".into(), args: vec![T::Int] };
    #[allow(unused_mut)]
    let mut decls = vec![
        ("sfx_tone",    vec![T::Int, T::Int, T::Int, T::Int],                  T::Unit),
        ("sfx_noise",   vec![T::Int, T::Int, T::Int],                          T::Unit),
        ("sfx_wave",    vec![T::Int, T::Int, T::Int, T::Int, T::Int],          T::Unit),
        ("sfx_inst",    vec![T::Int, T::Int, T::Int, T::Int, T::Int, T::Int],  T::Unit),
        ("sfx_pan",     vec![T::Int, T::Int, T::Int],                          T::Unit),
        ("sfx_fx",      vec![T::Int, T::Int, T::Int, T::Int],                  T::Unit),
        ("sfx_lfo",     vec![T::Int, T::Int, T::Int, T::Int, T::Int],          T::Unit),
        ("sfx_play",    vec![T::Int, T::Int, T::Int, T::Int],                  T::Unit),
        ("sfx_playm",   vec![T::Int, T::Int, T::Int, T::Int],                  T::Unit),
        ("sfx_off",     vec![T::Int],                                          T::Unit),
        ("sfx_seq",     vec![arr_int(), T::Int],                               T::Unit),
        ("sfx_track",   vec![arr_int(), T::Int],                               T::Unit),
        ("sfx_sample",  vec![arr_int(), T::Int, T::Int],                       T::Unit),
        ("sfx_samplestop", vec![],                                             T::Unit),
        ("sfx_seqstop", vec![],                                                T::Unit),
    ];
    #[cfg(feature = "fs")]
    {
        decls.push(("sfx_record_start", vec![T::String], T::Int));
        decls.push(("sfx_record_stop",  vec![],          T::Int));
    }
    decls
}
