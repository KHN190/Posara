#![cfg(feature = "sfx")]
use posara_core::sfx::{
    pcm::{Clip, Player},
    sample::SamplePlayer,
};
fn asset(rate: u32, n: usize) -> Vec<u8> {
    let mut b = b"PSM1".to_vec();
    b.extend(rate.to_le_bytes());
    b.extend((n as u32).to_le_bytes());
    for i in 0..n {
        b.extend((((i as f32 * 0.17).sin() * 20000.0) as i16).to_le_bytes());
    }
    b
}
#[test]
fn borrowed_and_vm_playback_match_for_all_rates() {
    for rate in [8000, 16000, 24000, 44100, 48000] {
        for output in [44100, 48000] {
            for gain in [0.0, 0.2, 1.0] {
                let b = asset(rate, 1000);
                let mut p = Player::default();
                p.play(Clip::parse(&b).unwrap(), output, gain);
                let mut vm = SamplePlayer::<posara_core::sfx::sample::Fixed>::default();
                vm.play(b.clone(), 0.0, gain);
                for _ in 0..10000 {
                    assert!((p.tick() - vm.tick(output as f32)).abs() < 0.0001);
                }
                assert!(!p.active());
            }
        }
    }
}
#[test]
fn malformed_pcm_is_rejected_without_reads_outside_payload() {
    let b = asset(24000, 100);
    for n in 0..b.len() {
        assert!(Clip::parse(&b[..n]).is_none());
        let mut p = SamplePlayer::<posara_core::sfx::sample::Fixed>::default();
        p.play(b[..n].to_vec(), 24000.0, 1.0);
        for _ in 0..1000 {
            assert!(p.tick(48000.0).is_finite());
        }
    }
    for rate in [0, 1, 7999, 96001, u32::MAX] {
        assert!(Clip::parse(&asset(rate, 10)).is_none());
    }
}
