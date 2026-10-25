use alloc::rc::Rc;
use alloc::sync::Arc;
use alloc::vec::Vec;

use super::runtime::{self, NativeCtx, Value, VirtualMachine, Int};

use crate::sfx::mixer::Cmd;
use crate::sfx::seq;
use crate::sfx::spsc::Spsc;
#[cfg(feature = "synth")]
use crate::sfx::synth::{MAX_UNISON, MAX_VOICES};
use super::abi::Args;

pub type CmdProd = Arc<Spsc<Cmd>>;

pub fn push(cmds: &CmdProd, cmd: Cmd) {
    let _ = cmds.try_push(cmd);
}

pub fn command(name: &str, a: &Args) -> Option<Cmd> {
    Some(match name {
        "snd_bus_delay" => Cmd::BusDelay(a.ms(0), a.percent(1), a.percent(2)),
        "snd_bus_reverb" => Cmd::BusReverb(a.percent(0), a.percent(1), a.percent(2)),
        "snd_seqstop" => Cmd::SeqStop,
        "snd_samplestop" => Cmd::SampleStop,
        #[cfg(feature = "synth")]
        "snd_osc" => Cmd::SynOsc(a.index(0), a.index(1), a.within(2, 0, 5) as u8, a.semitones(3) as i32, a.cents(4), a.percent(5)),
        #[cfg(feature = "synth")]
        "snd_filter" => Cmd::SynFilter(a.index(0), a.within(1, 0, 3) as u8, a.hz(2), a.within(3, 0, 100) as f32),
        #[cfg(feature = "synth")]
        "snd_env" => Cmd::SynEnv(a.index(0), a.index(1), a.within(2, 0, 2) as u8, a.level(3),
            a.ms(4) as f32 / 1000.0, a.ms(5) as f32 / 1000.0, a.percent(6), a.ms(7) as f32 / 1000.0),
        #[cfg(feature = "synth")]
        "snd_lfo" => Cmd::SynLfo(a.index(0), a.within(1, 0, 2) as u8, a.within(2, 0, 3) as u8, a.amount(3) / 100.0, a.percent(4)),
        #[cfg(feature = "synth")]
        "snd_pan" => Cmd::SynPan(a.index(0), a.balance(1)),
        #[cfg(feature = "synth")]
        "snd_unison" => Cmd::SynUnison(a.index(0), a.within(1, 1, MAX_UNISON as Int) as u8, a.cents(2)),
        #[cfg(feature = "synth")]
        "snd_fx" => Cmd::SynFx(a.index(0), a.within(1, 0, 5) as u8, a.percent(2), a.amount(3)),
        #[cfg(feature = "synth")]
        "snd_voices" => Cmd::SynVoices(a.within(0, 1, MAX_VOICES as Int) as usize),
        #[cfg(feature = "synth")]
        "snd_on" => Cmd::SynOn(a.index(0), a.within(1, 0, 127) as u8, a.percent(2), a.ms(3)),
        #[cfg(feature = "synth")]
        "snd_off" => Cmd::SynOff(a.index(0), a.within(1, 0, 127) as u8),
        #[cfg(feature = "synth")]
        "snd_stop" => Cmd::SynStop(a.index(0)),
        #[cfg(feature = "synth")]
        "snd_panic" => Cmd::SynPanic,
        _ => return None,
    })
}

fn control(vm: &mut VirtualMachine, names: &mut Vec<&'static str>, cmds: &CmdProd, list: &[&'static str]) {
    for &name in list {
        let p = Arc::clone(cmds);
        vm.register_native(name, Rc::new(move |_ctx: &mut NativeCtx, a: &[Value]| {
            if let Some(c) = command(name, &Args(a)) { push(&p, c); }
            ret_unit()
        }));
        names.push(name);
    }
}

fn ret_unit() -> Result<(Value, bool), alloc::string::String> { Ok((Value::UNIT, false)) }

// Ring full → command dropped; control rate is tiny vs the 1024-slot ring.
pub fn register(vm: &mut VirtualMachine, cmds: CmdProd) -> Vec<&'static str> {
    let mut names = Vec::new();
    control(vm, &mut names, &cmds, &["snd_bus_delay", "snd_bus_reverb", "snd_seqstop", "snd_samplestop"]);

    // .trk layout: 8 LE bytes per event, all-zero words are padding.
    let p = Arc::clone(&cmds);
    vm.register_native("snd_seq", Rc::new(move |ctx: &mut NativeCtx, a: &[Value]| {
        let a = Args(a);
        let data = a.handle(0);
        let ms_per_tick = a.ms(1).max(1);
        let Some(bytes) = runtime::read_bytes(ctx.heap, data) else { return ret_unit(); };
        push(&p, Cmd::Seq(seq::decode(&bytes), ms_per_tick));
        ret_unit()
    }));
    names.push("snd_seq");

    let p = Arc::clone(&cmds);
    vm.register_native("snd_sample", Rc::new(move |ctx: &mut NativeCtx, a: &[Value]| {
        let a = Args(a);
        let buf = a.handle(0);
        let rate = a.hz(1);
        let vol = a.percent(2);
        let Some(bytes) = runtime::read_bytes(ctx.heap, buf) else { return ret_unit(); };
        push(&p, Cmd::Sample(bytes, rate, vol));
        ret_unit()
    }));
    names.push("snd_sample");
    names
}

#[cfg(feature = "synth")]
pub fn register_synth(vm: &mut VirtualMachine, cmds: CmdProd) -> Vec<&'static str> {
    let mut names = Vec::new();
    control(vm, &mut names, &cmds, &["snd_osc", "snd_filter", "snd_env", "snd_lfo", "snd_pan", "snd_unison", "snd_fx",
        "snd_voices", "snd_on", "snd_off", "snd_stop", "snd_panic"]);
    names
}
