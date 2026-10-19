use posara_sync_jack::{Jack, SyncIn, SyncOut, CLOCK, START, STOP};

const BEAT_US: u64 = 500_000;

#[test]
fn clock_word_is_tagged_with_source() {
    assert_eq!(SyncIn::new(2).word(), CLOCK as u32 | 2 << 24);
}

#[test]
fn first_pulse_has_no_tempo_so_both_halves_arrive_at_once() {
    let mut s = SyncIn::new(0);
    assert_eq!(s.on_pulse(0), 6);
    assert_eq!(s.poll(0), 6);
    assert_eq!(s.poll(1), 0);
}

#[test]
fn second_half_arrives_at_the_midpoint_of_the_last_interval() {
    let mut s = SyncIn::new(0);
    s.on_pulse(0);
    s.poll(0);
    assert_eq!(s.on_pulse(BEAT_US), 6);
    assert_eq!(s.poll(BEAT_US), 0);
    assert_eq!(s.poll(BEAT_US + BEAT_US / 2 - 1), 0);
    assert_eq!(s.poll(BEAT_US + BEAT_US / 2), 6);
    assert_eq!(s.poll(BEAT_US + BEAT_US / 2 + 1), 0);
}

#[test]
fn every_pulse_yields_twelve_clocks() {
    let mut s = SyncIn::new(0);
    let mut total = 0u32;
    for i in 0..8u64 {
        let t = i * BEAT_US;
        total += s.on_pulse(t) as u32;
        for us in (t..t + BEAT_US).step_by(1000) { total += s.poll(us) as u32; }
    }
    assert_eq!(total, 8 * 12);
}

#[test]
fn early_pulse_flushes_the_pending_half_first() {
    let mut s = SyncIn::new(0);
    s.on_pulse(0);
    s.poll(0);
    s.on_pulse(BEAT_US);
    assert_eq!(s.on_pulse(BEAT_US + BEAT_US / 4), 12);
}

#[test]
fn stop_drops_the_pending_half_and_tempo() {
    let mut s = SyncIn::new(0);
    s.on_pulse(0);
    s.poll(0);
    s.on_pulse(BEAT_US);
    s.on_stop();
    assert_eq!(s.poll(10 * BEAT_US), 0);
    assert_eq!(s.on_pulse(20 * BEAT_US), 6);
    assert_eq!(s.poll(20 * BEAT_US), 6);
}

#[test]
fn out_pulses_every_twelve_clocks() {
    let mut s = SyncOut::new();
    let got: Vec<usize> = (0..25).filter(|_| s.on_send(CLOCK)).collect();
    assert_eq!(got, [0, 12, 24]);
}

#[test]
fn out_start_realigns_and_other_bytes_never_pulse() {
    let mut s = SyncOut::new();
    assert!(s.on_send(CLOCK));
    assert!(!s.on_send(CLOCK));
    assert!(!s.on_send(START));
    assert!(s.on_send(CLOCK));
    assert!(!s.on_send(STOP));
    assert!(!s.on_send(0x90));
}

const SECOND_US: u64 = 1_000_000;

fn plugged_at(t: u64) -> Jack {
    let mut j = Jack::new();
    j.plug(true, t);
    j
}

#[test]
fn unplugged_jack_leads() {
    assert!(Jack::new().leads());
}

#[test]
fn plugging_in_listens_for_a_second_then_leads() {
    let mut j = plugged_at(10);
    assert!(!j.leads());
    j.poll(10 + SECOND_US - 1);
    assert!(!j.leads());
    j.poll(10 + SECOND_US);
    assert!(j.leads());
}

#[test]
fn a_pulse_while_listening_makes_it_follow() {
    let mut j = plugged_at(0);
    j.on_pulse(SECOND_US / 2);
    j.poll(5 * SECOND_US / 2);
    assert!(!j.leads());
}

#[test]
fn a_leader_hearing_the_other_leader_yields() {
    let mut j = plugged_at(0);
    j.poll(SECOND_US);
    assert!(j.leads());
    j.on_pulse(2 * SECOND_US);
    assert!(!j.leads());
}

#[test]
fn a_follower_leads_after_three_silent_seconds() {
    let mut j = plugged_at(0);
    j.on_pulse(SECOND_US / 2);
    j.poll(SECOND_US / 2 + 3 * SECOND_US - 1);
    assert!(!j.leads());
    j.poll(SECOND_US / 2 + 3 * SECOND_US);
    assert!(j.leads());
}

#[test]
fn replugging_listens_again() {
    let mut j = plugged_at(0);
    j.poll(SECOND_US);
    j.plug(false, 2 * SECOND_US);
    j.plug(true, 3 * SECOND_US);
    assert!(!j.leads());
}

#[test]
fn a_steady_plug_state_does_not_restart_listening() {
    let mut j = plugged_at(0);
    j.poll(SECOND_US);
    j.plug(true, 2 * SECOND_US);
    assert!(j.leads());
}
