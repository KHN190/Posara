# System

## Time & random

```rust
now()         // millisecond timer (Int)
rand()        // random Float, [0, 1)
srand(seed)   // seed the RNG (commonly srand(now()))
```

Common idiom for an integer in `[0, n)`:

```rust
fn ri(n: Int) -> <nondet> Int { (rand() * n.to_f()).to_i() }
```

## Exit

```rust
halt(code)    // end the frame loop, return an exit code
```

## Console

```rust
println("hello")
```

## Files

Read and write files under the `--root` directory (fonts, sprites, samples, snapshots, etc.).

```rust
let fd   = fs_open("assets/mono8x16.fnt", 1);   // mode 1 = read
let data = fs_read(fd, 1520);                    // read N bytes
let _    = fs_close(fd);
```

Full set: `fs_open` / `fs_read` / `fs_read_text` / `fs_write` / `fs_write_text` / `fs_seek` / `fs_close`, plus `fs_exists` / `fs_list` / `fs_mkdir` / `fs_remove`.

Idiom: load once in `main` before the frame loop, keep the data in a local. See `carts/basic/text.abe` / `carts/basic/sprite.abe`.

## Effect annotations

The `<…>` on a function signature is its effect set, annotated by the capabilities it uses:

| Effect | Meaning |
|---|---|
| `<Graphics>` | drawing (cls / line / blitg …) |
| `<IO>` | audio, console, devices |
| `<nondet>` | uses `rand()` |

Composable, e.g. `-> <Graphics, IO> Unit`.
