use alloc::rc::Rc;

use myriad::{alloc_bytes, alloc_string, NativeCtx, Value, VirtualMachine};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ty { Int, Float, Bool, Str, Unit, Bytes, RefBytes, ArrInt, RefArrInt }

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Eff { Pure, Graphics, IO, Network, Nondet }

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cap { Core, Gfx, GfxFs, Input, Sfx, SfxFs, Synth, Fs, Midi, Net, Imu }

pub struct Sig {
    pub name: &'static str,
    pub params: &'static [Ty],
    pub ret: Ty,
    pub eff: Eff,
    pub cap: Cap,
}

use Cap::*;
use Eff::*;
use Ty::*;

const fn sig(name: &'static str, params: &'static [Ty], ret: Ty, eff: Eff, cap: Cap) -> Sig {
    Sig { name, params, ret, eff, cap }
}

pub static NATIVES: &[Sig] = &[
    sig("gfx_screen", &[Int, Int], Unit, Graphics, Gfx),
    sig("gfx_screen_off", &[], Unit, Graphics, Gfx),
    sig("gfx_commit", &[], Unit, Graphics, Gfx),
    sig("gfx_cls", &[Int], Unit, Graphics, Gfx),
    sig("gfx_win_pos", &[Int, Int], Unit, Graphics, Gfx),
    sig("gfx_pset", &[Int, Int, Int], Unit, Graphics, Gfx),
    sig("gfx_rect", &[Int, Int, Int, Int, Int], Unit, Graphics, Gfx),
    sig("gfx_rectb", &[Int, Int, Int, Int, Int], Unit, Graphics, Gfx),
    sig("gfx_rectmix", &[Int, Int, Int, Int, Int, Int], Unit, Graphics, Gfx),
    sig("gfx_dither", &[Int, Int], Unit, Graphics, Gfx),
    sig("gfx_line", &[Int, Int, Int, Int, Int], Unit, Graphics, Gfx),
    sig("gfx_linew", &[Int, Int, Int, Int, Int, Int], Unit, Graphics, Gfx),
    sig("gfx_circ", &[Int, Int, Int, Int], Unit, Graphics, Gfx),
    sig("gfx_circb", &[Int, Int, Int, Int], Unit, Graphics, Gfx),
    sig("gfx_tri", &[Int, Int, Int, Int, Int, Int, Int], Unit, Graphics, Gfx),
    sig("gfx_trib", &[Int, Int, Int, Int, Int, Int, Int], Unit, Graphics, Gfx),
    sig("gfx_pal", &[Int, Int], Unit, Graphics, Gfx),
    sig("gfx_blitg", &[RefBytes, Int, Int, Int, Int, Int, Int, Int], Unit, Graphics, Gfx),
    sig("gfx_blitr", &[RefBytes, Int, Int, Int, Int, Int, Int, Int], Unit, Graphics, Gfx),
    sig("gfx_sprite", &[RefBytes, Int, Int, Int, Int, Int, Int, Int], Unit, Graphics, Gfx),
    sig("gfx_save_png", &[Int, Int, Int, Int, Str], Int, IO, GfxFs),
    sig("in_buttons", &[], Int, IO, Input),
    sig("in_key", &[], Int, IO, Input),
    sig("snd_bus_delay", &[Int, Int, Int], Unit, IO, Sfx),
    sig("snd_bus_reverb", &[Int, Int, Int], Unit, IO, Sfx),
    sig("snd_seq", &[ArrInt, Int], Unit, IO, Sfx),
    sig("snd_track", &[Bytes, Int], Unit, IO, Sfx),
    sig("snd_sample", &[Bytes, Int, Int], Unit, IO, Sfx),
    sig("snd_samplestop", &[], Unit, IO, Sfx),
    sig("snd_seqstop", &[], Unit, IO, Sfx),
    sig("snd_bus_record_start", &[Str], Int, IO, SfxFs),
    sig("snd_bus_record_stop", &[], Int, IO, SfxFs),
    sig("snd_osc", &[Int, Int, Int, Int, Int, Int], Unit, IO, Synth),
    sig("snd_filter", &[Int, Int, Int, Int], Unit, IO, Synth),
    sig("snd_env", &[Int, Int, Int, Int, Int, Int, Int, Int], Unit, IO, Synth),
    sig("snd_lfo", &[Int, Int, Int, Int, Int], Unit, IO, Synth),
    sig("snd_pan", &[Int, Int], Unit, IO, Synth),
    sig("snd_unison", &[Int, Int, Int], Unit, IO, Synth),
    sig("snd_fx", &[Int, Int, Int, Int], Unit, IO, Synth),
    sig("snd_voices", &[Int], Unit, IO, Synth),
    sig("snd_on", &[Int, Int, Int, Int], Unit, IO, Synth),
    sig("snd_off", &[Int, Int], Unit, IO, Synth),
    sig("snd_stop", &[Int], Unit, IO, Synth),
    sig("snd_panic", &[], Unit, IO, Synth),
    sig("fs_exists", &[Str], Int, IO, Fs),
    sig("fs_mkdir", &[Str], Int, IO, Fs),
    sig("fs_remove", &[Str], Int, IO, Fs),
    sig("fs_list", &[Str], Str, IO, Fs),
    sig("fs_open", &[Str, Int], Int, IO, Fs),
    sig("fs_close", &[Int], Int, IO, Fs),
    sig("fs_seek", &[Int, Int, Int], Int, IO, Fs),
    sig("fs_read", &[Int, Int], Bytes, IO, Fs),
    sig("fs_reads", &[Int, Int], Str, IO, Fs),
    sig("fs_write", &[Int, ArrInt], Int, IO, Fs),
    sig("fs_writes", &[Int, Str], Int, IO, Fs),
    sig("midi_poll", &[], Int, IO, Midi),
    sig("midi_count", &[], Int, IO, Midi),
    sig("midi_send", &[Int], Unit, IO, Midi),
    sig("udp_bind", &[Int], Int, Network, Net),
    sig("udp_addr", &[Int], Str, Network, Net),
    sig("udp_recv", &[Int], Bytes, Network, Net),
    sig("udp_from", &[Int], Str, Network, Net),
    sig("udp_send", &[Int, Str, RefArrInt], Unit, Network, Net),
    sig("udp_broadcast", &[Int, Bool], Unit, Network, Net),
    sig("udp_close", &[Int], Unit, Network, Net),
    sig("imu_accel", &[Int], Int, IO, Imu),
    sig("now", &[], Int, IO, Core),
    sig("rand", &[], Float, Nondet, Core),
    sig("srand", &[Int], Unit, Nondet, Core),
    sig("assert", &[Bool, Str], Unit, Pure, Core),
];

pub fn names(cap: Cap) -> impl Iterator<Item = &'static str> {
    NATIVES.iter().filter(move |s| s.cap == cap).map(|s| s.name)
}

// Platforms override these with real natives; what stays answers 0 / "" / empty bytes.
pub fn stub_all(vm: &mut VirtualMachine) {
    for s in NATIVES {
        let ret = s.ret;
        vm.register_native(s.name, Rc::new(move |ctx: &mut NativeCtx, _: &[Value]| match ret {
            Unit => Ok((Value::UNIT, false)),
            Str => Ok((alloc_string(ctx.heap, "")?, true)),
            Bytes => Ok((alloc_bytes(ctx.heap, &[])?, true)),
            _ => Ok((Value::ZERO, false)),
        }));
    }
}
