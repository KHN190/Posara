# Tools

Offline asset converters. Two crates, off the default build — `cargo build`
builds only the runtime. Build or install them on demand:

```sh
cargo run -p posara-gfx -- <sub> ...      # or: cargo install --path tools/gfx
cargo run -p posara-sfx -- <sub> ...      #     cargo install --path tools/sfx
```

## posara-gfx — image → sprite

```sh
posara-gfx sprite <in.png> <out.spr> [opts]   # image → 1bpp / 4bpp sprite
posara-gfx font   <in.ttf> <out.fnt> [opts]   # ttf/otf → 1bpp glyph atlas
```

`sprite` opts: `--size WxH` · `--max N` · `--threshold T` · `--dither` ·
`--invert` (1bpp) · `--colors16` (4bpp, ≤15 pre-quantized colors, appends
frames) · `--quant` (4bpp, NeuQuant auto, index 0 transparent) · `--key RRGGBB`
· `--tol N`.

`font` opts: `--cell WxH` · `--px N` · `--first C` · `--count N` · `--baseline B`.

Cart loads: 1bpp `gfx_blitg(&spr, off, x, y, w, h, color, mode)`; 4bpp
`gfx_sprite(&spr, off, x, y, w, h, scale, alpha)` with `gfx_pal` from the `.pal`.

## posara-sfx — audio → data

```sh
posara-sfx sample <in.mp3> <out.sample> [--rate N]   # mp3 → 1-bit delta-sigma
posara-sfx track  <in.mid> <out.trk> [opts]          # midi → packed events
```

`track` opts: `--voices N` (1..4) · `--project voice|pitch` · `--transpose N` ·
`--res N` · `--tempo BPM`.

Cart loads: `snd_sample(&s, rate, vol)` · `snd_track(&t, ms_per_tick)`.
