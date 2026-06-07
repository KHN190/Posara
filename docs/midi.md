# MIDI

Posara connects to the first available MIDI input and output port the first time a cart touches device `0x90` (you'll see `• MIDI in: <name>` / `• MIDI out: <name>` on stderr). Carts that never use MIDI never open a port. Carts routed by `midi.toml` connect at startup instead, so their virtual source is visible for wiring.

## Receiving

```rust
let n  = device_out(0x9001);   // number of queued incoming events
let ev = device_out(0x9000);   // pop the oldest event (0 if the queue is empty)
```

Each event is a raw MIDI message packed into one integer:

```rust
ev = status + d1*256 + d2*65536
```

Unpack with division and `%`:

```rust
let status = ev % 256;          // e.g. 0x90..0x9F = note-on, 0x80..0x8F = note-off
let d1     = (ev / 256) % 256;  // note number
let d2     = (ev / 65536) % 256;// velocity
```

A note-on with velocity `0` means note-off — treat `status` in `0x90..0x9F` with `d2 == 0` like `0x80`.

Drain the queue once per frame:

```rust
let mut n = device_out(0x9001);
while n > 0 {
  let ev = device_out(0x9000);
  // ... handle ...
  n = n - 1
}
```

## Sending

```rust
device_in(0x9002, status + d1*256 + d2*65536)
```

Same packing as above. Examples:

```rust
device_in(0x9002, 0x90 + 60*256 + 100*65536)   // note-on  C4, velocity 100, channel 0
device_in(0x9002, 0x80 + 60*256)               // note-off C4, channel 0
```

Two-byte messages (program change `0xC0`, channel pressure `0xD0`) take `d1` only; the runtime sends the right length automatically.

If no MIDI output port exists, sends are silently dropped — carts don't need to guard.

## Tools

`midi2track` converts a standard MIDI file into an `.abe` track cart (see `carts/music/song.abe`, `priv_carts/glider.abe` for generated output).
