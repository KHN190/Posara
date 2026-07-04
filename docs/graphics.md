# Graphics

## Screen

```rust
screen(w, h)        // set canvas size
cls(color)          // clear the whole screen to a color
```

## Color — RGB565

A color is a 16-bit integer: 5 bits red, 6 bits green, 5 bits blue.

```rust
color = r * 2048 + g * 32 + b      // r,b ∈ 0..31  g ∈ 0..63
```

`0x0000` black, `0xffff` white. Hex literals work directly.

## Drawing primitives

```rust
pset(x, y, color)                       // single pixel
line(x0, y0, x1, y1, color)             // line segment
rect(x, y, w, h, color)                 // filled rectangle
rectb(x, y, w, h, color)                // rectangle border
circ(cx, cy, r, color)                  // filled circle
circb(cx, cy, r, color)                 // circle border
tri(x0, y0, x1, y1, x2, y2, color)      // filled triangle
trib(x0, y0, x1, y1, x2, y2, color)     // triangle border
linew(x0, y0, x1, y1, thick, color)     // thick line
```

> The `*b` suffix = border only; without it = filled. See `carts/mountain.abe` and `carts/lib/visuals.abe` for usage.

Advanced (blend / dither / palette / PNG): `rectmix(x,y,w,h,color,alpha)`, `dither(c1,c2)`, `pal(...)`, `blit` / `sprite` (PNG, needs `<IO>`), `save_png(path)` (needs `<IO>`).

## Bitmap blit (sprites / fonts)

```rust
blitg(data, bit_offset, x, y, w, h, color, mode)
```

Takes a `w×h` region from a 1bpp bitmap starting at `bit_offset`, draws it at `(x, y)`, tinted with `color`.

`mode = (rot << 4) | op`:

- `rot`: `0/1/2/3` = rotate `0/90/180/270`
- `op`: `0` REPLACE, `1` XOR

```rust
// 64×64 sprite, four orientations
blitg(SPR, 0,  30, 40, 64, 64, 0x0000, 0);    // 0°
blitg(SPR, 0, 130, 40, 64, 64, 0x0000, 16);   // 90°  (1<<4)
blitg(SPR, 0, 230, 40, 64, 64, 0x0000, 32);   // 180°

// 8×16 font glyph (128 bits each)
blitg(FONT, (cp - 32) * 128, x, y, 8, 16, color, mode)
```

Asset loading: see [system.md](system.md#files). Examples: `carts/basic/sprite.abe`, `carts/basic/text.abe`.

## API

All `<Graphics>` unless marked. Color = RGB565 Int.

Screen
- `screen(w, h)` — set canvas size.
- `screen_off()` — headless (no window).
- `cls(color)` — clear whole screen.
- `win_pos(x, y)` — place the window.
- `pal(idx: 0..15, rgb888)` — set palette entry (for indexed `sprite`).

Primitives
- `pset(x, y, color)` — one pixel.
- `line(x0, y0, x1, y1, color)` — segment.
- `linew(x0, y0, x1, y1, thick, color)` — thick line.
- `rect(x, y, w, h, color)` / `rectb(…)` — filled / border rectangle.
- `rectmix(x, y, w, h, color, alpha)` — alpha-blended rectangle.
- `circ(cx, cy, r, color)` / `circb(…)` — filled / border circle.
- `tri(x0,y0,x1,y1,x2,y2,color)` / `trib(…)` — filled / border triangle.
- `dither(c1, c2)` — set the 2-color dither pair.

Blit
- `blitg(data, bit_off, x, y, w, h, color, mode)` — 1bpp bitmap, tinted; `mode = (rot<<4)|op`, op `0` replace `1` XOR.
- `blitr(data, bit_off, x, y, w, h, color, mode)` — raw-color sibling of `blitg` (source carries color).
- `blit(data, x, y, w, h, color)` `<IO>` — draw a decoded PNG buffer.
- `sprite(&data, byte_off, x, y, w, h, scale, alpha)` `<IO>` — indexed sprite; scale = percent, alpha `0..256`.
- `save_png(x, y, w, h, path)` `<IO>` — dump a screen region to PNG.
