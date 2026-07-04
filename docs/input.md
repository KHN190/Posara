# Input

An 8-button bitmap plus the last ASCII key.

```rust
let buttons = device_out(0x8002);   // 8-button bitmap
let key     = device_out(0x8003);   // last ASCII key code
```

(Corresponds to controller ports 0x82 / 0x83.)

## Button bits

`buttons` is a bitmap; AND with a mask to test a key:

| bit (mask) | key |
|---|---|
| `0x01` | A |
| `0x02` | B |
| `0x04` | Select |
| `0x08` | Start |
| `0x10` | Up |
| `0x20` | Down |
| `0x40` | Left |
| `0x80` | Right |

## Example

How `carts/games/invader.abe` tests a bit (no bitwise operator — division + modulo):

```rust
fn bit_set(v: Int, mask: Int) -> Int { v / mask % 2 }

let b = device_out(0x8002);
if bit_set(b, 0x80) == 1 { /* Right */ };
if bit_set(b, 0x10) == 1 { /* Up */ };
if bit_set(b, 0x01) == 1 { /* A */ };
```

## API

Devices are port-addressed: `port = device_id << 8 | sub`. `device_out(port) -> Int` reads, `device_in(port, val)` writes. `<IO>`.

Controller (device `0x80`, read-only)
- `device_out(0x8002) -> Int` — 8-button bitmap (A `0x01` B `0x02` Select `0x04` Start `0x08` Up `0x10` Down `0x20` Left `0x40` Right `0x80`).
- `device_out(0x8003) -> Int` — last ASCII key code.

Screen (device `0x20`) and MIDI (device `0x90`) use the same `device_in`/`device_out` form — see [graphics.md](graphics.md) / [midi.md](midi.md).
