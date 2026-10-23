# Audio

A polyphonic synth (`snd_`): up to 16 patches sharing a 32-voice pool, plus a step
sequencer, sample playback, and a master bus.

Notes are MIDI numbers (0..127), volumes and most amounts 0..100, times in ms. A
patch `pid` is a reusable timbre — configure it once, then trigger many notes on it.

## Synth

A patch `pid` (0..15) is a timbre: two oscillators → filter → amp, shaped by two
envelopes, one LFO, optional unison and an insert effect.

```
osc0 + osc1  →  filter  →  ×amp  →  insert FX  →  pan → mix
   ▲              ▲                    ▲
 env0/env1       LFO              unison detune
```

### Voice pool

- `snd_voices(n)` — how many notes can sound at once across all patches.
  Default 8, max 32; beyond the pool the oldest voice is stolen. Global setting,
  call anytime.

### Build a patch

- `snd_osc(pid, idx, wave, semi, fine, level)` — set oscillator `idx` (0 or 1).
  `wave` 0 sq · 1 sin · 2 tri · 3 saw · 4 noise · 5 FM; `semi` pitch offset in semitones,
  `fine` detune in cents, `level` mix 0..100.
  `wave` 5 on osc 1 makes it a sine modulator: it shapes osc 0's phase instead of
  sounding, `level` sets the depth (100 = index 2). Values above 5 read as 5.
- `snd_filter(pid, kind, cutoff_hz, reso)` — one filter after the oscillators.
  `kind` 0 LP · 1 HP · 2 BP · 3 LPG; `cutoff_hz` corner frequency, `reso` 0..100.
- `snd_env(pid, slot, target, depth, atk, dec, sus, rel)` — one of two envelopes
  (`slot` 0 or 1) driving `target` 0 amp · 1 cutoff · 2 pitch. `depth` = how far it
  moves the target; `atk`/`dec`/`rel` in ms, `sus` level 0..100.
- `snd_lfo(pid, target, wave, rate_cHz, depth)` — one LFO on `target` 0 amp ·
  1 cutoff · 2 pitch (same order as env). `wave` 0..3 (same codes); `rate_cHz` in
  0.01 Hz steps; `depth` 0..100.
- `snd_unison(pid, count, detune_cents)` — stack `count` (1..7) detuned copies
  spread by `detune_cents` for a fatter sound.
- `snd_fx(pid, kind, amt, param)` — one insert effect. `kind` 1 bitcrush ·
  2 drive · 3 lopass · 4 hipass · 5 ring; `amt` 0..100; `param` = cutoff Hz
  (lopass/hipass) or ring frequency Hz.
- `snd_pan(pid, pos)` — stereo position, `pos` −100 left · 0 center · 100 right.

### Play

- `snd_on(pid, note, vol, dur_ms)` — trigger `note` at `vol` 0..100 for `dur_ms`.
- `snd_off(pid, note)` — release one held note early.
- `snd_stop(pid)` — release every note on a patch.
- `snd_panic()` — kill all voices immediately.

```rust
// 303-ish acid bass on patch 1.
snd_osc(1, 0, 3, 0, 0, 100);            // osc0, saw (wave 3), 0 semi/0 fine, level 100
snd_filter(1, 0, 520, 88);              // low-pass (kind 0), cutoff 520Hz, reso 88
snd_env(1, 0, 0, 100, 2, 260, 35, 80);  // slot0 → amp,    depth100, A2 D260 S35 R80
snd_env(1, 1, 1, 65, 2, 150, 0, 70);    // slot1 → cutoff, the squelch
snd_on(1, 45, 90, 130);                 // play MIDI 45
```

## Step sequencer

`snd_seq(data: Bytes, ms_per_tick)` queues `.trk` events; the audio thread fires them on a grid. The format is independent of the cart's integer width.

```rust
let events = b"\x00\x00\xe0\xd1\x0a\x00\x00\x00\x04\x00\xe0\xd1\x0a\x00\x00\x00";
snd_seq(events, 120);   // two notes, ticks 0 and 4
```

Each event is 8 little-endian bytes: tick bits 0–15, channel 16–18, note 19–26, volume 27–33, duration 34–49. Volume is 0..100; duration is in ticks. Use channels 0..3. All-zero events are padding; a partial final event is zero-padded. Events are sorted by tick, preserving order at the same tick.

- `snd_seqstop()` — stop sequencing. Empty sequence data also stops it.
- `snd_sample(data: Bytes, bit_rate_hz, vol)` — play MSB-first 1-bit delta-sigma `.sample` data; volume 0..100.
- `snd_samplestop()` — stop sample playback.

Generate `.trk` and `.sample` files with [posara-sfx](tools.md).

## Master bus

Global, applied to all patches.

- `snd_bus_delay(time_ms, feedback, mix)` — delay; `time_ms` tap time (up to 2000 on desktop and web), `feedback`/`mix` 0..100.
- `snd_bus_reverb(size, damp, mix)` — reverb; `size` room, `damp` high-freq damping, `mix` wet, all 0..100.
- `snd_bus_record_start(path: String) -> Int` — start WAV capture (needs `fs`).
- `snd_bus_record_stop() -> Int` — stop capture.

## Examples

- synth with visuals — `carts/vis/acid.abe`
- synth only — `carts/music/detroit.abe`, `dub.abe`, `electro.abe`
- MIDI in / out / routing — [midi.md](midi.md)
