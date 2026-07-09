#!/usr/bin/env bash
# wasm-target execution: builds posara-web for wasm and drives the real entry
# under node. Covers code mode, cart (.pk) mode, reload across screen sizes, an
# .abe reading an uploaded .spr asset from the sandbox, and cart stdout capture.
set -euo pipefail
cd "$(dirname "$0")/../crates/posara-web"
wasm-pack build --target nodejs --out-dir pkg-node >/dev/null 2>&1

cat > pkg-node/_smoke.mjs << 'EOF'
import { readFileSync } from 'fs';
import { Posara } from './posara_web.js';

const ok = (c, m) => { if (!c) { console.error("FAIL:", m); process.exit(1); } };
const CART128 = `@cart
fn main() -> <frame, Graphics, IO, nondet> Unit {
  gfx_screen(128, 128);
  loop { gfx_cls(0x0000); gfx_rect(8, 8, 16, 16, 0xF81F); gfx_commit(); frame.present() }
}`;
const CART_ASSET = `@cart
fn main() -> <frame, Graphics, IO> Unit {
  let fd = fs_open("data.spr", 1);
  let b = fs_read(fd, 2);
  let _ = fs_close(fd);
  gfx_screen(8, 8);
  loop { gfx_cls(b.byte_at(0) * 256 + b.byte_at(1)); gfx_commit(); frame.present() }
}`;

const p = new Posara();
ok(p.sample_rate() === 44100, "sample_rate");

// 1. code mode
p.load_src(CART128);
for (let i=0;i<3;i++) p.frame();
ok(p.width()===128 && p.height()===128, "code mode dims");
ok(p.framebuffer().some(b=>b!==0), "code mode blank frame");
console.log("code mode ok (128x128)");

// 2. reload to a .pk cart of a DIFFERENT size (framebuffer reset)
const pk = new Uint8Array(readFileSync(new URL('../../../carts/games/invader.pk', import.meta.url)));
p.add_file('invader.pk', pk);
p.load_pk(p.read_file('invader.pk'));
for (let i=0;i<3;i++) p.frame();
ok(p.width()===480 && p.height()===320, `reload dims got ${p.width()}x${p.height()}`);
console.log("cart mode + reload ok (480x320, multi-module .pk)");

// 3. .abe reads an uploaded .spr asset from the sandbox
p.add_file('data.spr', new Uint8Array([0xF8, 0x00]));   // → cls 0xF800
p.load_src(CART_ASSET);
for (let i=0;i<3;i++) p.frame();
ok(p.width()===8 && p.height()===8, "asset cart dims");
ok(p.framebuffer().some(b=>b!==0), "asset not read (blank = fs_read failed)");
console.log("multi-file ok (.abe reads uploaded .spr)");

// 4. cart stdout capture
p.load_src(`@cart
fn main() -> <frame, Graphics, IO> Unit {
  println("hello from cart");
  gfx_screen(4,4);
  loop { gfx_cls(0); gfx_commit(); frame.present() }
}`);
p.frame();
ok(p.drain_stdout().includes("hello from cart"), "stdout not captured");
console.log("stdout capture ok");

// 5. multi-module: main2.abe uses hello::{hi}
const E = s => new TextEncoder().encode(s);
p.add_file('hello.abe', E('pub fn hi() -> Int { 7 }\n'));
p.add_file('main2.abe', E('use hello::{hi}\n@cart\nfn main() -> <frame, Graphics, IO> Unit {\n  gfx_screen(hi() + 1, 8);\n  loop { gfx_cls(0); gfx_commit(); frame.present() }\n}\n'));
p.load_entry('main2.abe');
p.frame();
ok(p.width() === 8, `multi-module resolve got w=${p.width()}`);   // hi()+1 = 8
console.log("multi-module ok (use hello::{hi})");

// 6. missing module must error (the silent-accept bug fix)
let errored = false;
try {
  p.add_file('bad.abe', E('use nope::{x}\n@cart\nfn main() -> <frame, Graphics, IO> Unit {\n  x();\n  gfx_screen(4,4);\n  loop { gfx_cls(0); gfx_commit(); frame.present() }\n}\n'));
  p.load_entry('bad.abe');
} catch (e) { errored = true; }
ok(errored, "missing module should error, not silently pass");
console.log("missing-module error ok");

console.log("ALL WASM CHECKS PASSED");
EOF

node pkg-node/_smoke.mjs
rm -rf pkg-node
