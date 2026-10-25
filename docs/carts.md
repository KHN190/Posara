# Cart Structure

## The shape of a cart

An interactive cart is one `main` function marked `@cart`, with a `loop` inside. Set things up before the loop; the loop body runs once per frame (~60fps).

```rust
type World = { x: Int, prevb: Int }

@cart
fn main() -> <frame, Graphics, IO> Unit {
  gfx_screen(480, 320);
  let mut w = World { x: 0, prevb: 0 };
  loop {
    gfx_cls(0x0000);
    // ... update + draw ...
    gfx_commit();    // commit the frame — forget this and the window stays blank
    frame.present()
  }
}
```

Locals live across frames, so state is just a `World` record in `main`. Helpers take `&mut World`.

A plain `fn main() -> Unit` with no `@cart` runs once and exits — fine for scripts.

`halt(code)` ends the loop with an exit code.

## .abe vs .pk

- `.abe` — source, compiled on load. Develop with this; `posara run` hot-reloads it on save.
- `.pk` — precompiled cartridge, for distribution.

## Where things live

```sh
carts/
  posara.toml       ← project root marker
  midi.toml         ← MIDI wiring, optional (→ midi.md)
  assets/           ← fonts, sprites
  basic/  games/  music/  vis/
  lib/              ← shared code
```

`posara.toml` marks the root. `use` paths and `fs_open` paths both resolve from it, no matter how deep the cart sits. `--root <dir>` overrides.

## Imports

```rust
use lib::state::{ st_new, SNOW }     // → <root>/lib/state.abe
```

- `use a::b::{…}` maps to `<root>/a/b.abe`.
- Imports inside imported files also resolve from the root — `lib/visuals.abe` writes `use lib::state`, not `use state`.
- Importable: `pub fn`, `pub static` (immutable data), `pub effect`.

## Screen config

- `gfx_screen(w, h)` — set the canvas size.
- `gfx_commit()` — push the frame to the window (end every `update()` with it).

Raw Screen device writes (`DEO`, device `0x20`) follow the cartridge profile:

- `INT32_SAFE`: port `0x2000` packs width in bits 0–15 and height in bits 16–31. Format defaults to `1` (RGB565); port `0x2002` explicitly selects it. Unsupported formats fail before configuration.
- 64-bit: port `0x2000` keeps the existing width16/height16/format8 layout; format occupies bits 32–39. Port `0x2002` is unsupported.

Port `0x2001` commits in both profiles. The opcode and cartridge wire formats are unchanged.
