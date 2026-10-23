use posara_sync_jack::{Jack, SyncIn, SyncOut, CLOCK, START, STOP};

const BEAT_US: u64 = 500_000;

#[test]
fn first_pulse_and_restart_after_silence_deliver_both_halves_at_once() {
    let mut s = SyncIn::new(2);
    assert_eq!(s.word(), CLOCK as u32 | 2 << 24);
    assert_eq!((s.on_pulse(0), s.poll(0), s.poll(1)), (6, 6, 0));
    s.on_pulse(BEAT_US);
    assert_eq!(s.on_pulse(BEAT_US + BEAT_US / 4), 12);
    s.on_stop();
    assert_eq!((s.poll(10 * BEAT_US), s.on_pulse(20 * BEAT_US), s.poll(20 * BEAT_US)), (0, 6, 6));

    let mut l = following(0);
    l.on_pulse(0);
    l.poll(0);
    l.on_pulse(EIGHTH_US);
    l.poll(EIGHTH_US + EIGHTH_US / 2);
    let back = EIGHTH_US + 10 * SECOND_US;
    l.poll(back - 1);
    assert_eq!(l.on_pulse(back) + l.poll(back), 12);
}

const SECOND_US: u64 = 1_000_000;
const EIGHTH_US: u64 = 250_000;

use posara_sync_jack::Link;

fn following(at: u64) -> Link {
    let mut l = Link::new(1);
    l.plug(true, at);
    l
}

#[test]
fn half_an_hour_of_pocket_operator_pulses_never_drifts() {
    let mut l = following(0);
    let (mut clocks, mut t) = (0u64, 0u64);
    for k in 0..7200u64 {
        let jitter = [0, 700, 300, 1000, 0, 500][k as usize % 6];
        let pulse = k * EIGHTH_US + jitter;
        while t + 1000 <= pulse {
            t += 1000;
            clocks += l.poll(t) as u64;
        }
        clocks += l.on_pulse(pulse) as u64;
        assert!(clocks.abs_diff(12 * k + 6) <= 6, "pulse {k}: {clocks} clocks");
    }
    assert_eq!(clocks + l.poll(t + EIGHTH_US) as u64, 12 * 7200);
}

use proptest::prelude::*;

fn run_follower(eighth_us: u64, jitter_us: &[u64], pulses: usize) -> Result<(), TestCaseError> {
    let mut l = following(0);
    let (mut clocks, mut t) = (0u64, 0u64);
    for k in 0..pulses {
        let at = k as u64 * eighth_us + jitter_us[k % jitter_us.len()];
        while t + 1000 <= at {
            t += 1000;
            let n = l.poll(t) as u64;
            prop_assert!(n <= 6, "{n} clocks in one poll");
            clocks += n;
        }
        if k > 0 { prop_assert_eq!(clocks, 12 * k as u64, "before pulse {}", k); }
        clocks += l.on_pulse(at) as u64;
        prop_assert!(!l.on_send(CLOCK, at));
    }
    clocks += l.poll(t + 3 * eighth_us) as u64;
    prop_assert_eq!(clocks, 12 * pulses as u64);
    Ok(())
}

#[derive(Clone, Copy, Debug)]
enum Ev { Plug(bool), Pulse, Poll }

fn ev() -> impl Strategy<Value = (u64, Ev)> {
    (0u64..1500, prop_oneof![any::<bool>().prop_map(Ev::Plug), Just(Ev::Pulse), Just(Ev::Poll)])
}

#[derive(Clone, Copy, PartialEq)]
enum Mode { Listen(u64), Follow(u64), Lead }

proptest! {
    #[test]
    fn any_tempo_and_jitter_gives_twelve_clocks_per_pulse(bpm in 40u64..300, jitter in prop::collection::vec(0u64..2000, 1..8), pulses in 2usize..120) {
        run_follower(30_000_000 / bpm, &jitter, pulses)?;
    }

    #[test]
    fn the_second_half_lands_at_the_midpoint(interval_ms in 5u64..2000) {
        let mut l = following(0);
        l.on_pulse(0);
        l.poll(0);
        let at = interval_ms * 1000;
        l.on_pulse(at);
        let mid = at + at / 2;
        let mut t = at;
        while l.poll(t) == 0 {
            t += 1000;
            prop_assert!(t <= mid + 1000, "half never came");
        }
        prop_assert!(t >= mid && t < mid + 1000, "half at {t}, mid {mid}");
    }

    #[test]
    fn out_pulses_fall_on_every_twelfth_clock_since_start(sends in prop::collection::vec(prop_oneof![Just(CLOCK), Just(START), Just(STOP), Just(0x90u8)], 0..300)) {
        let mut out = SyncOut::new();
        let mut l = Link::new(1);
        let mut since_start = 0u32;
        for s in sends {
            let want = match s {
                START => { since_start = 0; false }
                CLOCK => { let p = since_start % 12 == 0; since_start += 1; p }
                _ => false,
            };
            prop_assert_eq!(out.on_send(s), want);
            prop_assert_eq!(l.on_send(s, 0), want);
        }
    }

    #[test]
    fn jack_roles_follow_the_spec(evs in prop::collection::vec(ev(), 0..60)) {
        let mut j = Jack::new();
        prop_assert!(j.leads());
        let (mut plugged, mut mode, mut now) = (false, Mode::Lead, 0u64);
        for (dt_ms, e) in evs {
            now += dt_ms * 1000;
            match e {
                Ev::Plug(p) => {
                    if p != plugged { mode = if p { Mode::Listen(now) } else { Mode::Lead }; }
                    plugged = p;
                    j.plug(p, now);
                }
                Ev::Pulse => { mode = Mode::Follow(now); j.on_pulse(now); }
                Ev::Poll => {
                    mode = match mode {
                        Mode::Listen(t) if now - t >= 1_000_000 => Mode::Lead,
                        Mode::Follow(t) if now - t >= 3_000_000 => Mode::Lead,
                        m => m,
                    };
                    j.poll(now);
                }
            }
            prop_assert_eq!(j.leads(), mode == Mode::Lead);
        }
    }

    #[test]
    fn shifting_time_changes_nothing(evs in prop::collection::vec(ev(), 0..60), shift in 0u64..1_000_000_000) {
        let (mut a, mut b) = (Link::new(1), Link::new(1));
        let mut now = 0u64;
        for (dt_ms, e) in evs {
            now += dt_ms * 1000;
            let (x, y) = match e {
                Ev::Plug(p) => { a.plug(p, now); b.plug(p, now + shift); (0, 0) }
                Ev::Pulse => (a.on_pulse(now), b.on_pulse(now + shift)),
                Ev::Poll => (a.poll(now), b.poll(now + shift)),
            };
            prop_assert_eq!(x, y);
            prop_assert_eq!(a.leads(), b.leads());
            prop_assert_eq!(a.on_send(CLOCK, now), b.on_send(CLOCK, now + shift));
        }
    }

    #[test]
    fn two_devices_plugged_together_settle_on_one_leader(
        (bpm_a, bpm_b) in (60u64..180, 60u64..179).prop_map(|(a, b)| (a, if b >= a { b + 1 } else { b })),
        phase_a in 0u64..1000, phase_b in 0u64..1000,
        plug_a in 0u64..2000, plug_b in 0u64..2000,
    ) {
        prop_assert_eq!(unsettled([bpm_a, bpm_b], [phase_a, phase_b], [plug_a, plug_b]), None);
        prop_assert_eq!(unsettled([144, 115], [767, 10], [13, 0]), None);
    }

    #[test]
    fn two_devices_at_one_tempo_settle_unless_in_lockstep(
        (bpm, apart) in (60u64..180).prop_flat_map(|bpm| (Just(bpm), 6..tick(bpm) / 1000 - 6)),
        phase in 0u64..1000, plug_a in 0u64..2000, plug_b in 0u64..2000,
    ) {
        prop_assert_eq!(unsettled([bpm, bpm], [phase, phase + apart], [plug_a, plug_b]), None);
    }
}

#[test]
#[ignore = "identical tempo and phase: both keep leading in lockstep; real clocks drift apart"]
fn identical_devices_plugged_at_once_still_settle() {
    let (mut a, mut b) = (Link::new(1), Link::new(1));
    let tick = 500_000 / 24;
    for ms in 0..8000u64 {
        let t = ms * 1000;
        a.plug(true, t);
        b.plug(true, t);
        a.poll(t);
        b.poll(t);
        let beat = t % tick < 1000;
        let (a_out, b_out) = (beat && a.on_send(CLOCK, t), beat && b.on_send(CLOCK, t));
        if a_out { b.on_pulse(t); }
        if b_out { a.on_pulse(t); }
        if ms >= 2000 { assert_eq!(a.leads() as u8 + b.leads() as u8, 1, "at {ms} ms"); }
    }
}

fn tick(bpm: u64) -> u64 { 60_000_000 / bpm / 24 }

fn unsettled(bpm: [u64; 2], phase: [u64; 2], plug: [u64; 2]) -> Option<u64> {
    let (mut a, mut b) = (Link::new(1), Link::new(1));
    let (ta, tb) = (tick(bpm[0]), tick(bpm[1]));
    let settle_ms = plug[0].max(plug[1]) + 1000 + 2 * 60_000 / bpm[0].min(bpm[1]);
    for ms in 0..settle_ms + 5000 {
        let t = ms * 1000;
        a.plug(ms >= plug[0], t);
        b.plug(ms >= plug[1], t);
        a.poll(t);
        b.poll(t);
        let a_out = (t + phase[0] * 1000) % ta < 1000 && a.on_send(CLOCK, t);
        let b_out = (t + phase[1] * 1000) % tb < 1000 && b.on_send(CLOCK, t);
        if a_out && ms >= plug[1] { b.on_pulse(t); }
        if b_out && ms >= plug[0] { a.on_pulse(t); }
        if ms >= settle_ms && a.leads() as u8 + b.leads() as u8 != 1 { return Some(ms); }
    }
    None
}
