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

Full set: `fs_open` / `fs_read` / `fs_reads` / `fs_write` / `fs_writes` / `fs_seek` / `fs_close`, plus `fs_exists` / `fs_list` / `fs_mkdir` / `fs_remove`.

Idiom: load once in `main` before the frame loop, keep the data in a local. See `carts/basic/text.abe` / `carts/basic/sprite.abe`.

## Effect annotations

The `<…>` on a function signature is its effect set, annotated by the capabilities it uses:

| Effect | Meaning |
|---|---|
| `<Graphics>` | drawing (cls / line / blitg …) |
| `<IO>` | audio, console, devices |
| `<nondet>` | uses `rand()` |

Composable, e.g. `-> <Graphics, IO> Unit`.

## API

Time & random
- `now() -> Int` `<IO>` — millisecond timer.
- `rand() -> Float` `<nondet>` — random in `[0, 1)`.
- `srand(seed: Int)` `<IO>` — seed the RNG.

Control & console
- `halt(code: Int)` `<IO>` — end the frame loop, return exit code.
- `println(s: String)` `<IO>` — write line to stdout.

Files (all `<IO>`; `fd` from `fs_open`, `-1` on error)
- `fs_open(path: String, mode: Int) -> Int` — open, return fd. mode = OR of `1` read `2` write `4` create `8` append `16` truncate.
- `fs_read(fd, n: Int) -> Array<Int>` — read n bytes, zero-padded past EOF (arrays have no len).
- `fs_reads(fd, n: Int) -> String` — read n bytes as UTF-8 text.
- `fs_write(fd, data: Array<Int>) -> Int` — write bytes, return count.
- `fs_writes(fd, s: String) -> Int` — write text, return byte count.
- `fs_seek(fd, off: Int, whence: Int) -> Int` — move cursor, return new pos. whence `0` start `1` cur `2` end; `fs_seek(fd,0,2)` = file size.
- `fs_close(fd) -> Int` — close fd.
- `fs_exists(path: String) -> Int` — `1` if exists else `0`.
- `fs_list(dir: String) -> String` — newline-joined sorted names (carts can't parse String; needs byte-returning variant to be usable).
- `fs_mkdir(path: String) -> Int` — create directory.
- `fs_remove(path: String) -> Int` — delete file.
