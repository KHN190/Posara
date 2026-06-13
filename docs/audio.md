# Audio

Two sets of instruments to play notes on.

- **synth** — rich polyphonic synth: up to 16 patches, up to 32 notes at once.
  For full, layered music — pianos, pads, basses together.
- **sfx** — chiptune kit: up to 4 voices, plus samples and a step sequencer.
  For retro bleeps, hits, or a tight 4-track feel.

The 4-voice limit is **sfx** only. **synth** has no such limit, so layering many
sounds is normal, not a trick.

## Synth

```rust
synth_voices(n)                                    // voice pool size, max 32

synth_osc(pid, idx, wave, semi, fine, level)       // osc idx 0|1
synth_filter(pid, kind, cutoff_hz, reso)
synth_env(pid, slot, target, depth, atk, dec, sus, rel)   // slot 0|1, times in ms
synth_lfo(pid, target, rate_cHz, depth)
synth_unison(pid, count, detune_cents)             // count 1..7
synth_fx(pid, kind, amt, param)

synth_on(pid, note, vol, dur_ms)                   // trigger a note
synth_off(pid, note)                               // release a held note
synth_stop(pid)                                     // release all notes on a patch
synth_panic()                                       // kill every voice
```

A patch (`pid` 0..15) is a fixed voice graph; configure it once, then play it by
id. Each `synth_on` allocates one of the shared voices (oldest stolen when full).

```
osc0 + osc1  →  filter  →  ×amp  →  insert FX  →  mix
   ▲              ▲                    ▲
 env0/env1       LFO              unison detune
```

- **osc** ×2 — `wave` `0` square · `1` sine · `2` triangle · `3` saw · `4` noise;
  `semi` + `fine` (cents) detune; `level` 0..100.
- **filter** — `kind` `0` low-pass · `1` high-pass · `2` band-pass · `3` LPG;
  `cutoff_hz`, `reso` 0..100.
- **env** ×2 — each routes to a `target`: `0` amp · `1` cutoff · `2` pitch;
  `depth` scales the amount, then ADSR in ms (`sus` 0..100).
- **lfo** ×1 — `target` `0` pitch · `1` amp · `2` cutoff; `rate_cHz` in units of
  0.01 Hz; `depth`.
- **unison** — stack `count` detuned copies, spread by `detune_cents`.
- **fx** — one insert per patch: `kind` `1` bitcrush · `2` drive · `3` lopass ·
  `4` hipass · `5` ring; `amt` 0..100, `param` = cutoff / ring Hz.

```rust
// 303-ish acid bass on patch 1.
synth_osc(1, 0, 3, 0, 0, 100);            // saw
synth_filter(1, 0, 520, 88);              // resonant low-pass
synth_env(1, 0, 0, 100, 2, 260, 35, 80);  // amp ADSR
synth_env(1, 1, 1, 65, 2, 150, 0, 70);    // cutoff env = the squelch
synth_on(1, 45, 90, 130);                 // play MIDI 45
```

## sfx

```rust
sfx_inst(ch, wave, atk_ms, dec_ms, sus, rel_ms)    // configure an ADSR voice
sfx_playm(ch, note, vol, dur_ms)                   // play a MIDI note on it
sfx_tone(freq_hz, dur_ms, vol, ch)                 // one-shot at a raw frequency
sfx_pan(ch, l, r)                                  // 0..100 each side
sfx_fx(ch, kind, amt, param)                       // same fx kinds as synth
sfx_lfo(ch, target, wave, rate_cHz, depth)
sfx_sample(...)                                     // PCM sample playback
sfx_seq([ ev0, ev1, ... ])                          // queue sequencer events
sfx_track(...)                                       // load a generated track
```

`wave` codes match the synth. Fire-and-forget; lighter than a synth patch.

### Step sequencer

Queue events in one call; the audio thread fires them on the grid. Pack each
event into an integer:

```rust
fn ev(tick, ch, note, vol, dur) -> Int {
  tick + ch*65536 + note*524288 + vol*134217728 + dur*17179869184
}
sfx_seq([ ev(0, 0, 36, 90, 2), ev(4, 0, 36, 90, 2) ])
```

Field shifts: `tick` (low) · `ch ×65536` · `note ×524288` · `vol ×134217728` ·
`dur ×17179869184`. `sfx_track` loads output from `midi2track`.

## Examples

- synth with visuals — `carts/vis/acid.abe`
- synth only - `carts/music/detroit.abe`, `dub.abe`, `electro.abe`
- MIDI in / out / routing — [midi.md](midi.md)
