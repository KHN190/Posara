# Input

An 8-button bitmap plus the last ASCII key.

```rust
let buttons = in_buttons();   // 8-button bitmap
let key     = in_key();       // last ASCII key code
```

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

```rust
type Btn = A | B | Select | Start | Up | Down | Left | Right;

fn btn_mask(b: Btn) -> Int {
  match b {
    A => 0x01, B => 0x02, Select => 0x04, Start => 0x08,
    Up => 0x10, Down => 0x20, Left => 0x40, Right => 0x80,
  }
}
fn keydown(bits: Int, b: Btn) -> Bool { (bits & btn_mask(b)) != 0 }

let bits = in_buttons();
if keydown(bits, Up) { /* Up */ };
if keydown(bits, A)  { /* A  */ };
```

## API

- `in_buttons() -> Int` `<IO>` — 8-button bitmap.
- `in_key() -> Int` `<IO>` — last ASCII key code.
