use posara::sfx::mixer::Cmd;
use proptest::prelude::*;

#[derive(Clone, Debug)]
pub enum Op {
    Inst(usize, u8, u32, u32, f32, u32),
    Pan(usize, f32, f32),
    Fx(usize, u8, f32, f32),
    Lfo(usize, u8, u8, f32, f32),
    Play(usize, f32, f32, u32),
    PlayMidi(usize, i64, f32, u32),
    Off(usize),
    Tone(f32, u32, f32, u32),
    Noise(u32, f32, u32),
    Wave(u8, f32, u32, f32, u32),
    BusDelay(u32, f32, f32),
    BusReverb(f32, f32, f32),
    SynOsc(usize, usize, u8, i64, f32, f32),
    SynFilter(usize, u8, f32, f32),
    SynEnv(usize, usize, u8, f32, f32, f32, f32, f32),
    SynLfo(usize, u8, u8, f32, f32),
    SynPan(usize, f32),
    SynUnison(usize, u8, f32),
    SynFx(usize, u8, f32, f32),
    SynVoices(usize),
    SynOn(usize, u8, f32, u32),
    SynOff(usize, u8),
    SynStop(usize),
    SynPanic,
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
            Op::BusDelay(a, b, c) => Cmd::BusDelay(a, b, c),
            Op::BusReverb(a, b, c) => Cmd::BusReverb(a, b, c),
            Op::SynOsc(a, b, c, d, e, f) => Cmd::SynOsc(a, b, c, d, e, f),
            Op::SynFilter(a, b, c, d) => Cmd::SynFilter(a, b, c, d),
            Op::SynEnv(a, b, c, d, e, f, g, h) => Cmd::SynEnv(a, b, c, d, e, f, g, h),
            Op::SynLfo(a, b, c, d, e) => Cmd::SynLfo(a, b, c, d, e),
            Op::SynPan(a, b) => Cmd::SynPan(a, b),
            Op::SynUnison(a, b, c) => Cmd::SynUnison(a, b, c),
            Op::SynFx(a, b, c, d) => Cmd::SynFx(a, b, c, d),
            Op::SynVoices(a) => Cmd::SynVoices(a),
            Op::SynOn(a, b, c, d) => Cmd::SynOn(a, b, c, d),
            Op::SynOff(a, b) => Cmd::SynOff(a, b),
            Op::SynStop(a) => Cmd::SynStop(a),
            Op::SynPanic => Cmd::SynPanic,
        }
    }

    #[allow(dead_code)]
    pub fn is_bus_delay(&self) -> bool { matches!(self, Op::BusDelay(..)) }
}

fn level() -> impl Strategy<Value = f32> { prop_oneof![0.0f32..=1.0, -2.0f32..4.0] }
fn wide() -> impl Strategy<Value = f32> { prop_oneof![-1e6f32..1e6, -200.0f32..20000.0] }
fn secs() -> impl Strategy<Value = f32> { 0.0f32..2.0 }
fn ms() -> impl Strategy<Value = u32> { 0u32..3000 }

pub fn op() -> impl Strategy<Value = Op> {
    let ch = 0usize..5;
    let pid = 0usize..18;
    prop_oneof![
        (ch.clone(), 0u8..6, ms(), ms(), level(), ms()).prop_map(|(a, b, c, d, e, f)| Op::Inst(a, b, c, d, e, f)),
        (ch.clone(), level(), level()).prop_map(|(a, b, c)| Op::Pan(a, b, c)),
        (ch.clone(), 0u8..7, level(), wide()).prop_map(|(a, b, c, d)| Op::Fx(a, b, c, d)),
        (ch.clone(), 0u8..4, 0u8..5, wide(), level()).prop_map(|(a, b, c, d, e)| Op::Lfo(a, b, c, d, e)),
        (ch.clone(), wide(), level(), ms()).prop_map(|(a, b, c, d)| Op::Play(a, b, c, d)),
        (ch.clone(), -20i64..150, level(), ms()).prop_map(|(a, b, c, d)| Op::PlayMidi(a, b, c, d)),
        ch.clone().prop_map(Op::Off),
        (wide(), ms(), level(), ms()).prop_map(|(a, b, c, d)| Op::Tone(a, b, c, d)),
        (ms(), level(), ms()).prop_map(|(a, b, c)| Op::Noise(a, b, c)),
        (0u8..6, wide(), ms(), level(), ms()).prop_map(|(a, b, c, d, e)| Op::Wave(a, b, c, d, e)),
        (ms(), level(), level()).prop_map(|(a, b, c)| Op::BusDelay(a, b, c)),
        (level(), level(), level()).prop_map(|(a, b, c)| Op::BusReverb(a, b, c)),
        (pid.clone(), 0usize..3, 0u8..6, -200i64..200, wide(), level()).prop_map(|(a, b, c, d, e, f)| Op::SynOsc(a, b, c, d, e, f)),
        (pid.clone(), 0u8..5, wide(), wide()).prop_map(|(a, b, c, d)| Op::SynFilter(a, b, c, d)),
        (pid.clone(), 0usize..3, 0u8..4, wide(), secs(), secs(), level(), secs()).prop_map(|(a, b, c, d, e, f, g, h)| Op::SynEnv(a, b, c, d, e, f, g, h)),
        (pid.clone(), 0u8..4, 0u8..5, wide(), level()).prop_map(|(a, b, c, d, e)| Op::SynLfo(a, b, c, d, e)),
        (pid.clone(), level()).prop_map(|(a, b)| Op::SynPan(a, b)),
        (pid.clone(), 0u8..10, wide()).prop_map(|(a, b, c)| Op::SynUnison(a, b, c)),
        (pid.clone(), 0u8..7, level(), wide()).prop_map(|(a, b, c, d)| Op::SynFx(a, b, c, d)),
        (0usize..40).prop_map(Op::SynVoices),
        (pid.clone(), any::<u8>(), level(), ms()).prop_map(|(a, b, c, d)| Op::SynOn(a, b, c, d)),
        (pid.clone(), any::<u8>()).prop_map(|(a, b)| Op::SynOff(a, b)),
        pid.prop_map(Op::SynStop),
        Just(Op::SynPanic),
    ]
}

pub fn session() -> impl Strategy<Value = Vec<(Op, usize)>> {
    proptest::collection::vec((op(), 0usize..6), 1..40)
}
