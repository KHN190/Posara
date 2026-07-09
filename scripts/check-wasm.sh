#!/usr/bin/env bash
# wasm-target execution smoke: builds posara-web for wasm and runs the real entry
# (new/load_src/frame/framebuffer/audio_pull) under node. Catches wasm-only traps
# (e.g. std::time on wasm) that native tests can't see.
set -euo pipefail
cd "$(dirname "$0")/../crates/posara-web"
wasm-pack build --target nodejs --out-dir pkg-node >/dev/null 2>&1

cat > pkg-node/_smoke.mjs << 'EOF'
import { Posara } from './posara_web.js';
const DEMO = `@cart
fn main() -> <frame, Graphics, IO, nondet> Unit {
  gfx_screen(128,128);
  let mut t = 0;
  loop { gfx_cls(0x0000); gfx_rect(t % 112, 56, 16, 16, 0xF81F); gfx_commit(); t = t + 2; frame.present() }
}
`;
const p = new Posara();                       // <- time panic would fire here
if (p.sample_rate() !== 44100) throw new Error("sample_rate");
p.load_src(DEMO);
for (let i = 0; i < 3; i++) p.frame();
if (p.width() !== 128 || p.height() !== 128) throw new Error("dims");
const fb = p.framebuffer();
if (fb.length !== 128*128*4) throw new Error("fb size");
if (!fb.some(b => b !== 0)) throw new Error("blank frame");
const a = new Float32Array(256); p.audio_pull(a);
console.log("wasm smoke: OK");
EOF

node pkg-node/_smoke.mjs
rm -rf pkg-node
