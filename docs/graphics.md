# Graphics

## Screen

```rust
gfx_screen(w, h)        // set canvas size
gfx_cls(color)          // clear the whole screen to a color
```

## Color — RGB565

A color is a 16-bit integer: 5 bits red, 6 bits green, 5 bits blue.

```rust
color = r * 2048 + g * 32 + b      // r,b ∈ 0..31  g ∈ 0..63
```

`0x0000` black, `0xffff` white. Hex literals work directly.

## Drawing primitives

```rust
gfx_pset(x, y, color)                       // single pixel
gfx_line(x0, y0, x1, y1, color)             // line segment
gfx_rect(x, y, w, h, color)                 // filled rectangle
gfx_rectb(x, y, w, h, color)                // rectangle border
gfx_circ(cx, cy, r, color)                  // filled circle
gfx_circb(cx, cy, r, color)                 // circle border
gfx_tri(x0, y0, x1, y1, x2, y2, color)      // filled triangle
gfx_trib(x0, y0, x1, y1, x2, y2, color)     // triangle border
gfx_linew(x0, y0, x1, y1, thick, color)     // thick line
```

> The `*b` suffix = border only; without it = filled. See `carts/mountain.abe` and `carts/lib/visuals.abe` for usage.

Advanced (blend / dither / palette / commit / PNG): `gfx_rectmix(x,y,w,h,color,alpha)`, `gfx_dither(c1,c2)`, `gfx_pal(...)`, `gfx_blitg` / `gfx_sprite` (below), `gfx_commit()`, `gfx_save_png(path)` (needs `<IO>`).

## Bitmap blit (sprites / fonts)

```rust
gfx_blitg(data, bit_offset, x, y, w, h, color, mode)
```

Takes a `w×h` region from a 1bpp bitmap starting at `bit_offset`, draws it at `(x, y)`, tinted with `color`.

`mode = (rot << 4) | op`:

- `rot`: `0/1/2/3` = rotate `0/90/180/270`
- `op`: `0` REPLACE, `1` XOR

```rust
// 64×64 sprite, four orientations
gfx_blitg(SPR, 0,  30, 40, 64, 64, 0x0000, 0);    // 0°
gfx_blitg(SPR, 0, 130, 40, 64, 64, 0x0000, 16);   // 90°  (1<<4)
gfx_blitg(SPR, 0, 230, 40, 64, 64, 0x0000, 32);   // 180°

// 8×16 font glyph (128 bits each)
gfx_blitg(FONT, (cp - 32) * 128, x, y, 8, 16, color, mode)
```

Asset loading: see [system.md](system.md#files). Examples: `carts/basic/sprite.abe`, `carts/basic/text.abe`.

## API

All `<Graphics>` unless marked. Color = RGB565 Int.

Screen
- `gfx_screen(w, h)` — set canvas size.
- `gfx_screen_off()` — headless (no window).
- `gfx_cls(color)` — clear whole screen.
- `gfx_win_pos(x, y)` — place the window.
- `gfx_pal(idx: 0..15, rgb888)` — set palette entry (for indexed `sprite`).

Primitives
- `gfx_pset(x, y, color)` — one pixel.
- `gfx_line(x0, y0, x1, y1, color)` — segment.
- `gfx_linew(x0, y0, x1, y1, thick, color)` — thick line.
- `gfx_rect(x, y, w, h, color)` / `gfx_rectb(…)` — filled / border rectangle.
- `gfx_rectmix(x, y, w, h, color, alpha)` — alpha-blended rectangle.
- `gfx_circ(cx, cy, r, color)` / `gfx_circb(…)` — filled / border circle.
- `gfx_tri(x0,y0,x1,y1,x2,y2,color)` / `gfx_trib(…)` — filled / border triangle.
- `gfx_dither(c1, c2)` — set the 2-color dither pair.

Blit
- `gfx_blitg(data: Array<Int>, bit_off, x, y, w, h, color, mode)` — 1bpp bitmap, tinted; `mode = (rot<<4)|op`, op `0` replace `1` XOR.
- `gfx_blitr(data: Array<Int>, x, y, w, h, color, mode, deg)` — arbitrary-angle rotation (slow path).
- `gfx_sprite(&data: Bytes, byte_off, x, y, w, h, scale, alpha)` — indexed 4bpp sprite; feed a `fs_readb` sheet. scale = percent, alpha `0..256`.
- `gfx_save_png(x, y, w, h, path)` `<IO>` — dump a screen region to PNG.
