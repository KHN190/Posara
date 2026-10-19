use alloc::rc::Rc;
use alloc::sync::Arc;
use alloc::vec::Vec;

use myriad::{NativeCtx, Value, VirtualMachine};

use super::mixer::Cmd;
use super::seq;
use super::spsc::Spsc;

pub type CmdProd = Arc<Spsc<Cmd>>;

pub fn push(cmds: &CmdProd, cmd: Cmd) {
    let _ = cmds.try_push(cmd);
}

macro_rules! native {
    ($vm:ident, $names:ident, $cmds:ident, $name:literal, |$a:ident| $cmd:expr) => {{
        let p = Arc::clone(&$cmds);
        $vm.register_native($name, Rc::new(move |_ctx: &mut NativeCtx, $a: &[Value]| {
            push(&p, $cmd);
            ret_unit()
        }));
        $names.push($name);
    }};
}

fn arg(args: &[Value], i: usize) -> i64 {
    args.get(i).copied().unwrap_or(Value::ZERO).as_int()
}

fn pct(args: &[Value], i: usize) -> f32 { (arg(args, i).clamp(0, 100) as f32) / 100.0 }

fn ret_unit() -> Result<(Value, bool), alloc::string::String> { Ok((Value::UNIT, false)) }

// Ring full → command dropped; control rate is tiny vs the 1024-slot ring.
pub fn register(vm: &mut VirtualMachine, cmds: CmdProd) -> Vec<&'static str> {
    let mut names = Vec::new();
    native!(vm, names, cmds, "snd_bus_delay", |a| Cmd::BusDelay(arg(a, 0).max(0) as u32, pct(a, 1), pct(a, 2)));
    native!(vm, names, cmds, "snd_bus_reverb", |a| Cmd::BusReverb(pct(a, 0), pct(a, 1), pct(a, 2)));

    let p = Arc::clone(&cmds);
    vm.register_native("snd_seq", Rc::new(move |ctx: &mut NativeCtx, a: &[Value]| {
        let pattern = a.first().copied().unwrap_or(Value::NONE);
        let ms_per_tick = arg(a, 1).max(1) as u32;
        if pattern.is_handle_none() { return ret_unit(); }
        let (slot, gen_) = pattern.as_handle();
        let cells = ctx.heap.cell_data(slot, gen_)?;
        let events: Vec<seq::Event> = cells.iter().map(|&w| seq::unpack(w as i64)).collect();
        push(&p, Cmd::Seq(seq::by_tick(events), ms_per_tick));
        ret_unit()
    }));
    names.push("snd_seq");
    // .trk layout: 8 LE bytes per event, all-zero words are padding.
    let p = Arc::clone(&cmds);
    vm.register_native("snd_track", Rc::new(move |ctx: &mut NativeCtx, a: &[Value]| {
        let data = a.first().copied().unwrap_or(Value::NONE);
        let ms_per_tick = arg(a, 1).max(1) as u32;
        let Some(bytes) = myriad::read_bytes(ctx.heap, data) else { return ret_unit(); };
        let mut events: Vec<seq::Event> = Vec::with_capacity(bytes.len() / 8);
        for chunk in bytes.chunks(8) {
            let mut word: u64 = 0;
            for (b, &c) in chunk.iter().enumerate() {
                word |= (c as u64) << (8 * b);
            }
            if word != 0 { events.push(seq::unpack(word as i64)); }
        }
        push(&p, Cmd::Seq(seq::by_tick(events), ms_per_tick));
        ret_unit()
    }));
    names.push("snd_track");
    native!(vm, names, cmds, "snd_seqstop", |_a| Cmd::SeqStop);

    let p = Arc::clone(&cmds);
    vm.register_native("snd_sample", Rc::new(move |ctx: &mut NativeCtx, a: &[Value]| {
        let buf = a.first().copied().unwrap_or(Value::NONE);
        let rate = arg(a, 1).max(1) as f32;
        let vol = pct(a, 2);
        let Some(bytes) = myriad::read_bytes(ctx.heap, buf) else { return ret_unit(); };
        push(&p, Cmd::Sample(bytes, rate, vol));
        ret_unit()
    }));
    names.push("snd_sample");
    native!(vm, names, cmds, "snd_samplestop", |_a| Cmd::SampleStop);
    names
}

#[cfg(feature = "synth")]
pub fn register_synth(vm: &mut VirtualMachine, cmds: CmdProd) -> Vec<&'static str> {
    let mut names = Vec::new();
    native!(vm, names, cmds, "snd_osc", |a| Cmd::SynOsc(arg(a, 0).max(0) as usize, arg(a, 1).max(0) as usize,
        arg(a, 2).clamp(0, 5) as u8, arg(a, 3), arg(a, 4) as f32, pct(a, 5)));
    native!(vm, names, cmds, "snd_filter", |a| Cmd::SynFilter(arg(a, 0).max(0) as usize, arg(a, 1).clamp(0, 3) as u8,
        arg(a, 2).max(0) as f32, arg(a, 3) as f32));
    native!(vm, names, cmds, "snd_env", |a| Cmd::SynEnv(arg(a, 0).max(0) as usize, arg(a, 1).max(0) as usize,
        arg(a, 2).clamp(0, 2) as u8, arg(a, 3) as f32, arg(a, 4).max(0) as f32 / 1000.0,
        arg(a, 5).max(0) as f32 / 1000.0, pct(a, 6), arg(a, 7).max(0) as f32 / 1000.0));
    native!(vm, names, cmds, "snd_lfo", |a| Cmd::SynLfo(arg(a, 0).max(0) as usize, arg(a, 1).clamp(0, 2) as u8,
        arg(a, 2).clamp(0, 3) as u8, arg(a, 3).max(0) as f32 / 100.0, pct(a, 4)));
    native!(vm, names, cmds, "snd_pan", |a| Cmd::SynPan(arg(a, 0).max(0) as usize, (arg(a, 1).clamp(-100, 100) as f32) / 100.0));
    native!(vm, names, cmds, "snd_unison", |a| Cmd::SynUnison(arg(a, 0).max(0) as usize, arg(a, 1).clamp(1, 7) as u8, arg(a, 2) as f32));
    native!(vm, names, cmds, "snd_fx", |a| Cmd::SynFx(arg(a, 0).max(0) as usize, arg(a, 1).clamp(0, 5) as u8,
        pct(a, 2), arg(a, 3).max(0) as f32));
    native!(vm, names, cmds, "snd_voices", |a| Cmd::SynVoices(arg(a, 0).max(1) as usize));
    native!(vm, names, cmds, "snd_on", |a| Cmd::SynOn(arg(a, 0).max(0) as usize, arg(a, 1).clamp(0, 127) as u8,
        pct(a, 2), arg(a, 3).max(0) as u32));
    native!(vm, names, cmds, "snd_off", |a| Cmd::SynOff(arg(a, 0).max(0) as usize, arg(a, 1).clamp(0, 127) as u8));
    native!(vm, names, cmds, "snd_stop", |a| Cmd::SynStop(arg(a, 0).max(0) as usize));
    native!(vm, names, cmds, "snd_panic", |_a| Cmd::SynPanic);
    names
}
