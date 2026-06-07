# Audio

4 channels (ch 0–3), each an oscillator + envelope. Configure instruments in `start()`, trigger notes in `update()`.

## Instruments & playback

```rust
sfx_inst(ch, wave, atk_ms, dec_ms, sus, rel_ms)
```
Configure a channel's voice (ADSR). `wave`: `0` square / `1` sine / `2` triangle / `3` saw / `4` noise.

```rust
sfx_playm(ch, note, vol, dur_ms)
```
Play a MIDI `note` on a channel at volume `vol` for `dur_ms`.

```rust
sfx_tone(freq_hz, dur_ms, vol, ch)
```
One-shot tone at a given frequency (no `sfx_inst` needed first).

## Pan / fx / LFO

```rust
sfx_pan(ch, l, r)                 // left/right volume 0..100
sfx_fx(ch, kind, amt, param)      // kind: 1 bitcrush 2 drive 3 lopass 4 hipass 5 ring
                                  // param = cutoff Hz / ring Hz
sfx_lfo(ch, target, wave, rate_cHz, depth)   // rate in units of 0.01Hz
```

## Sequencing

Queue a list of events into the sequencer at once:

```rust
sfx_seq([ ev0, ev1, ... ])
```

Each event is a value packed into an integer. `ride.abe` / `sound.abe` use a helper to pack one:

```rust
fn ev(tick, ch, note, vol, dur) -> Int {
  tick + ch*65536 + note*524288 + vol*134217728 + dur*17179869184
}
```

Field shifts: `tick` (low bits), `ch ×65536`, `note ×524288`, `vol ×134217728`, `dur ×17179869184`.

## Example

```rust
pub fn start() -> <IO> Unit {
  sfx_inst(0, 1, 80, 800, 70, 4500);   // ch0: sine, slow attack, long release (pad / pedal)
  sfx_pan(0, 100, 100)
}

pub fn update() -> <IO, nondet> Unit {
  sfx_playm(0, 44, 50, 6000)           // play MIDI 44
}
```

> Full examples: `carts/sound.abe` (key-driven demo of each API), `carts/music/ride.abe`, `carts/music/song.abe`.
