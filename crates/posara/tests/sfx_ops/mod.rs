use myriad::Value;
use posara::sfx::mixer::Cmd;
use posara_core::abi::{Args, Cap, NATIVES};
use posara_core::sfx::natives::command;
use proptest::prelude::*;

#[derive(Clone, Debug)]
pub enum Op {
    Inst(usize, u8, u32, u32, f32, u32),
    Pan(usize, f32, f32),
    Fx(usize, u8, f32, f32),
    Lfo(usize, u8, u8, f32, f32),
    Play(usize, f32, f32, u32),
    PlayMidi(usize, i32, f32, u32),
    Off(usize),
    Tone(f32, u32, f32, u32),
    Noise(u32, f32, u32),
    Wave(u8, f32, u32, f32, u32),
    Native(&'static str, Vec<i64>),
}

impl Op {
    pub fn cmd(&self) -> Cmd {
        match *self {
            Op::Inst(a, b, c, d, e, f) => Cmd::Inst(a, b, c, d, e, f),
            Op::Pan(a, b, c) => Cmd::Pan(a, b, c),
            Op::Fx(a, b, c, d) => Cmd::Fx(a, b, c, d),
            Op::Lfo(a, b, c, d, e) => Cmd::Lfo(a, b, c, d, e),
            Op::Play(a, b, c, d) => Cmd::Play(a, b, c, d),
            Op::PlayMidi(a, b, c, d) => Cmd::PlayMidi(a, b, c, d),
            Op::Off(a) => Cmd::Off(a),
            Op::Tone(a, b, c, d) => Cmd::Tone(a, b, c, d),
            Op::Noise(a, b, c) => Cmd::Noise(a, b, c),
            Op::Wave(a, b, c, d, e) => Cmd::Wave(a, b, c, d, e),
            Op::Native(name, ref args) => {
                let vals: Vec<Value> = args.iter().map(|&n| Value::from_int(n)).collect();
                command(name, &Args(&vals)).unwrap()
            }
        }
    }

    #[allow(dead_code)]
    pub fn is_bus_delay(&self) -> bool { matches!(self, Op::Native("snd_bus_delay", _)) }
}

fn level() -> impl Strategy<Value = f32> { prop_oneof![0.0f32..=1.0, -2.0f32..4.0] }
fn wide() -> impl Strategy<Value = f32> { prop_oneof![-1e6f32..1e6, -200.0f32..20000.0] }
fn ms() -> impl Strategy<Value = u32> { 0u32..3000 }

pub fn op() -> impl Strategy<Value = Op> {
    let ch = 0usize..5;
    prop_oneof![
        (ch.clone(), 0u8..6, ms(), ms(), level(), ms()).prop_map(|(a, b, c, d, e, f)| Op::Inst(a, b, c, d, e, f)),
        (ch.clone(), level(), level()).prop_map(|(a, b, c)| Op::Pan(a, b, c)),
        (ch.clone(), 0u8..7, level(), wide()).prop_map(|(a, b, c, d)| Op::Fx(a, b, c, d)),
        (ch.clone(), 0u8..4, 0u8..5, wide(), level()).prop_map(|(a, b, c, d, e)| Op::Lfo(a, b, c, d, e)),
        (ch.clone(), wide(), level(), ms()).prop_map(|(a, b, c, d)| Op::Play(a, b, c, d)),
        (ch.clone(), -20i32..150, level(), ms()).prop_map(|(a, b, c, d)| Op::PlayMidi(a, b, c, d)),
        ch.clone().prop_map(Op::Off),
        (wide(), ms(), level(), ms()).prop_map(|(a, b, c, d)| Op::Tone(a, b, c, d)),
        (ms(), level(), ms()).prop_map(|(a, b, c)| Op::Noise(a, b, c)),
        (0u8..6, wide(), ms(), level(), ms()).prop_map(|(a, b, c, d, e)| Op::Wave(a, b, c, d, e)),
        native(),
    ]
}

pub fn session() -> impl Strategy<Value = Vec<(Op, usize)>> {
    proptest::collection::vec((op(), 0usize..6), 1..40)
}

fn native() -> impl Strategy<Value = Op> {
    let names: Vec<&'static str> = NATIVES.iter()
        .filter(|s| matches!(s.cap, Cap::Sfx | Cap::Synth) && command(s.name, &Args(&[])).is_some())
        .map(|s| s.name).collect();
    (proptest::sample::select(names), proptest::collection::vec(prop_oneof![-300i64..300, any::<i64>(), Just(i64::MIN), Just(i64::MAX)], 8))
        .prop_map(|(name, args)| Op::Native(name, args))
}
