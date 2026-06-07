# Cart Structure

## Entry functions

| Function | When called | Notes |
|---|---|---|
| `fn main() -> Unit` | required | runs for one-shot carts; interactive carts write `()` |
| `pub fn start()` | once after load | setup: screen, instruments |
| `pub fn update()` | every frame (~60fps, 16.667ms) | main loop: draw, play, read input |

- If `update` exists → frame loop; otherwise `main` / run-once and exit.
- `halt(code)` ends the loop and returns an exit code.

```rust
fn main() -> Unit { () }
pub fn start()  -> <Graphics> Unit { screen(480, 320) }
pub fn update() -> <Graphics, IO> Unit { cls(0x0000) }
```

## .abe vs .pk

- `.abe` — source, compiled by the built-in compiler on load. Use during development.
- `.pk` — precompiled cartridge (polka format). Use for distribution.

## Module system

Use `use` to import `pub` items from another file, in the same directory or a subdirectory:

```rust
use lib::state::{ INIT, AXIS }      // → <root>/lib/state.abe
use lib::visuals::{ vis_anchor }    // → <root>/lib/visuals.abe
```

Rules:

- `use a::b::c::{…}` maps to `<root>/a/b/c.abe` (leading segments are directories, the last is the file name).
- `<root>` = the **entry cart's directory**. **All** imports resolve relative to that directory — including `use` statements inside imported files. So `lib/visuals.abe` referencing `state` must also write `use lib::state`, not `use state`.
- Only `pub` items can be imported: `pub static mut` (shared state), `pub fn`, `pub effect`.

### Conventional layout

```sh
carts/
  main.abe          ← demo / entry cart
  lib/
    state.abe         ← shared state
    visuals.abe       ← shared code
```

Entry carts sit at the top of `carts/`; reusable libraries go in `carts/lib/` and are imported via `use lib::xxx`.

## Screen config

Either:

- `screen(w, h)` — set the canvas size.
- `device_in(0x2000, cfg)` — same, with scale: `cfg = w + h*65536 + scale*4294967296`.
