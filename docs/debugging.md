# Debugging

Tools for finding out why a cart misbehaves, roughly in the order you should reach for them.

## Check first

```sh
posara check carts/games/surf.abe
```

Compiles the cart and prints lint warnings without running it. Always the first step after an edit — most "bugs" are caught here.

## Run a few frames headless

```sh
posara bench --frames 60 carts/games/surf.abe        # run 60 frames, no window, no vsync
posara dump --headless --at-frame 3 --out f3.png carts/games/surf.abe
```

`bench` runs the frame loop as fast as possible — good for "does it crash" and for profilers. `dump` runs to a given frame (or `--at-ms`) and saves the framebuffer as a PNG, so you can see what the cart drew without opening a window.

## Trace execution

```sh
posara run --trace    carts/games/surf.abe    # every opcode, with function name and pc
posara run --handlers carts/games/surf.abe    # effect handler push/resume only
posara run --debug    carts/games/surf.abe    # both
posara run --leak     carts/games/surf.abe    # dump live heap slots on exit
```

`--trace` is verbose — at 60fps it floods stderr. Prefer it with `bench --frames 1`, or skip straight to a breakpoint.

## Breakpoints

Set the `BREAK_AT` environment variable to `<fn>:<pc>` and run any command that executes the cart:

```sh
BREAK_AT=step:6 posara bench --frames 2 carts/games/surf.abe
```

Every time execution reaches that opcode, posara dumps the function's registers:

```
[break step#99:6] Div(Register(8), Register(6), Register(7)) (base r15)
    r0   = 0x0000000001000000  (handle)
    r6   = 0x0000000000000000
    r7   = 0x0000000000000040
```

- `<fn>` is the function name, or `#<id>` to break by function id (useful for unnamed or duplicated names).
- `<pc>` is the opcode index inside that function — get it from `disasm` (below).
- Registers marked `(handle)` point into the VM heap; the rest are raw values (ints, floats as bits).
- Works with `run`, `bench`, `dump`, `record` and `test`, and combines with `--trace`.

## Disassemble

```sh
posara disasm carts/games/surf.abe
```

Prints every function's bytecode with constants, call targets and static names annotated. Use it to pick a `pc` for `BREAK_AT`, or to see what the optimizer actually emitted.

```
fn #99 step (regs=40, consts=34)
     0: Ld(Register(2), Register(0), 0)
     ...
     6: Div(Register(8), Register(6), Register(7))
```

## Tests

```sh
posara test carts/games/surf.abe
```

Runs every `pub fn test_*()` export, each in a fresh VM. Use `assert(cond, "msg")` inside tests; a failed assert reports the message and the test fails. Headless — graphics calls are safe but nothing is shown, and a test can't catch a missing screen commit.

## Profiling

```sh
posara run --profile carts/games/surf.abe     # on-screen overlay: ops, heap, frame time, audio
PROFILE=1 posara bench --frames 600 cart.abe  # text summary at exit
```

If a frame exceeds the ops budget, posara prints a rate-limited warning (`ops/frame ... > budget ...`) — that's the signal to profile before players feel the stutter.
