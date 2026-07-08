# MIDI

Posara connects to the first available MIDI input and output port the first time a cart touches device `0x90` (you'll see `• MIDI in: <name>` / `• MIDI out: <name>` on stderr). Carts that never use MIDI never open a port. Carts routed by `midi.toml` connect at startup instead, so their virtual source is visible for wiring.

## Receiving

```rust
let n  = midi_count();   // number of queued incoming events
let ev = midi_poll();   // pop the oldest event (0 if the queue is empty)
```

Each event is a raw MIDI message packed into one integer:

```rust
ev = status + d1*256 + d2*65536 + src*16777216
```

Unpack with division and `%`:

```rust
let status = ev % 256;             // e.g. 0x90..0x9F = note-on, 0x80..0x8F = note-off
let d1     = (ev / 256) % 256;     // note number
let d2     = (ev / 65536) % 256;   // velocity
let src    = ev / 16777216;        // which wired-in source sent it (0 when unrouted)
```

A note-on with velocity `0` means note-off — treat `status` in `0x90..0x9F` with `d2 == 0` like `0x80`.

Drain the queue once per frame:

```rust
let mut n = midi_count();
while n > 0 {
  let ev = midi_poll();
  // ... handle ...
  n = n - 1
}
```

## Sending

```rust
midi_send(status + d1*256 + d2*65536)
```

Same packing as above. Examples:

```rust
midi_send(0x90 + 60*256 + 100*65536)   // note-on  C4, velocity 100, channel 0
midi_send(0x80 + 60*256)               // note-off C4, channel 0
```

Two-byte messages (program change `0xC0`, channel pressure `0xD0`) take `d1` only; the runtime sends the right length automatically.

With routing, bits 24+ pick the destination wire (`0xFF` = broadcast); plain sends go to the first. Missing destination = silently dropped.

## Routing — midi.toml

`midi.toml` next to `posara.toml` wires carts and devices; cart code unchanged.

```toml
[nodes]
seq = { cart = "music/sequencer.abe" }   # cart, by entry path
op1 = { port = "OP-1*" }                 # external device, by glob

[[wires]]
from = "seq"
to   = ["op1"]
```

Start order free (missing ends retry every 2s). Undeclared carts fall back to first-port mode. Example: `carts/midi.toml`; design: `designs/midi.md`.

## Tools

`midi2track` converts a standard MIDI file into an `.abe` track cart (see `carts/music/song.abe`). Live examples: `carts/music/sequencer.abe`, `carts/music/tracker.abe`.

## API

Ports open the first time a cart calls any `midi_*` native. `<IO>`.

- `midi_poll() -> Int` — pop oldest incoming event, `0` if queue empty.
- `midi_count() -> Int` — queued event count (drain once per frame).
- `midi_send(msg)` — send a message: `msg = status | d1<<8 | d2<<16` (dest chosen by routing).

Event/message packing: `status | d1<<8 | d2<<16 | src<<24` (incoming carries `src`). Unpack with `/` and `%`.
