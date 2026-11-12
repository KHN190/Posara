#![cfg(feature="synth")]
use posara_core::sfx::synth::Synth;
fn synth(pid:usize)->Synth{let mut s=Synth::new();s.set_voices(3);s.osc(pid,0,1,0,0.0,1.0);s.osc(pid,1,5,12,0.0,0.4);s.env(pid,0,0,100.0,0.001,0.1,0.7,0.1);s.note_on(pid,48,0.5,0);s}
#[test]fn temporary_modulation_is_patch_local_and_neutral_is_bit_identical(){
 for target in [[0.0,1.0,0.0],[1.0,0.8,0.0],[1.0,1.0,0.8],[1.0,1.0,0.0]] {for pid in [0,1,2,15]{
 let mut a=synth(pid);let mut b=synth(pid);a.modulate(0,target[0],target[1],target[2]);let mut difference=0.0;
 for _ in 0..8000{let x=a.tick(48000.0);let y=b.tick(48000.0);assert!(x.0.is_finite()&&x.1.is_finite());difference+=(x.0-y.0).abs();if pid!=0||target==[1.0,1.0,0.0]{assert_eq!(x,y);}}
 if pid==0&&target!=[1.0,1.0,0.0]{assert!(difference>1.0);}
 }}
}
#[test]fn mute_recovers_without_retriggering_and_invalid_controls_do_not_poison_audio(){for gain in [0.0,0.2,0.5]{let mut a=synth(0);a.modulate(0,gain,1.0,0.0);for _ in 0..4000{a.tick(48000.0);}a.modulate(0,1.0,1.0,0.0);let mut energy=0.0;for _ in 0..4000{energy+=a.tick(48000.0).0.abs();}assert!(energy>1.0);for bad in [f32::NAN,f32::INFINITY,f32::NEG_INFINITY]{a.modulate(0,bad,bad,bad);assert!(a.tick(48000.0).0.is_finite());}}}
