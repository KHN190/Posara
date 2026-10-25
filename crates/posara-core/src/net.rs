use core::cell::RefCell;

use super::runtime::{alloc_bytes, alloc_string, read_string, NativeCtx, Value, VirtualMachine, Int};

use super::abi::Args;
use crate::prelude::*;

pub const MAX_SOCKETS: usize = 8;

pub type Net = Rc<RefCell<Box<dyn Sockets>>>;

// One contract for both backends so the real/stub impls can't silently drift.
// A None return means the id is invalid (stale, never bound, or a -1 bind
// failure the cart forgot to check) — the native turns that into a runtime
// error so the bug surfaces instead of a silent no-op. recv() fills an internal
// buffer and returns its length (Some(0) = no packet, a normal poll); recv_buf()
// then reads it — no per-call return allocation, the native copies to the heap.
pub trait Sockets {
    fn bind(&mut self, port: u16) -> Int;
    fn addr(&self, id: Int) -> Option<String>;
    fn recv(&mut self, id: Int) -> Option<usize>;
    fn recv_buf(&self) -> &[u8];
    fn from(&self, id: Int) -> Option<String>;
    fn send(&self, id: Int, addr: &str, data: &[u8]) -> Option<()>;
    fn broadcast(&self, id: Int, on: bool) -> Option<()>;
    fn close(&mut self, id: Int);          // tolerant: closing an invalid id is a no-op
    fn sweep(&mut self);
    fn live(&self) -> usize;
}

fn ret_int(n: Int) -> Result<(Value, bool), String> { Ok((Value::from_int(n), false)) }
fn ret_unit() -> Result<(Value, bool), String> { Ok((Value::UNIT, false)) }
fn bad_sock(id: Int) -> String { format!("udp: invalid socket {id} (stale, unbound, or bind failed)") }

pub struct NullSockets { next: Option<Int> }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn null_socket_ids_retire_at_the_profile_limit() {
        let mut sockets = NullSockets { next: Some(Int::MAX) };
        assert_eq!(sockets.bind(0), Int::MAX);
        assert_eq!(sockets.bind(0), -1);
        assert_eq!(sockets.bind(0), -1);
    }
}

impl NullSockets {
    pub fn new() -> Self { Self { next: Some(0) } }
}

impl Default for NullSockets {
    fn default() -> Self { Self::new() }
}

// Every op succeeds inertly — a cart runs without net, never erroring.
impl Sockets for NullSockets {
    fn live(&self) -> usize { 0 }
    fn bind(&mut self, _port: u16) -> Int { let Some(id) = self.next else { return -1; }; self.next = id.checked_add(1); id }
    fn addr(&self, _id: Int) -> Option<String> { Some(String::new()) }
    fn recv(&mut self, _id: Int) -> Option<usize> { Some(0) }
    fn recv_buf(&self) -> &[u8] { &[] }
    fn from(&self, _id: Int) -> Option<String> { Some(String::new()) }
    fn send(&self, _id: Int, _addr: &str, _data: &[u8]) -> Option<()> { Some(()) }
    fn broadcast(&self, _id: Int, _on: bool) -> Option<()> { Some(()) }
    fn close(&mut self, _id: Int) {}
    fn sweep(&mut self) {}
}

pub fn register_natives(vm: &mut VirtualMachine, net: Net) {
    let t = Rc::clone(&net);
    vm.register_native("udp_bind", Rc::new(move |_: &mut NativeCtx, a: &[Value]| {
        let p = Args(a).word(0);
        if !(0..=65535).contains(&p) { return ret_int(-1); }
        ret_int(t.borrow_mut().bind(p as u16))
    }));
    let t = Rc::clone(&net);
    vm.register_native("udp_addr", Rc::new(move |ctx: &mut NativeCtx, a: &[Value]| {
        let id = Args(a).word(0);
        let s = t.borrow().addr(id).ok_or_else(|| bad_sock(id))?;
        Ok((alloc_string(ctx.heap, &s)?, true))
    }));
    let t = Rc::clone(&net);
    vm.register_native("udp_recv", Rc::new(move |ctx: &mut NativeCtx, a: &[Value]| {
        let id = Args(a).word(0);
        let mut tb = t.borrow_mut();
        tb.recv(id).ok_or_else(|| bad_sock(id))?;           // fills tb's buffer
        Ok((alloc_bytes(ctx.heap, tb.recv_buf())?, true))   // one copy: buffer → heap
    }));
    let t = Rc::clone(&net);
    vm.register_native("udp_from", Rc::new(move |ctx: &mut NativeCtx, a: &[Value]| {
        let id = Args(a).word(0);
        let s = t.borrow().from(id).ok_or_else(|| bad_sock(id))?;
        Ok((alloc_string(ctx.heap, &s)?, true))
    }));
    // data is a borrowed Array<Int> byte buffer — one element per byte, sent
    // whole (arr.len() bytes). Borrowed (&), so the cart reuses the buffer across
    // sends with no realloc; sized exactly to the packet, so no length arg.
    let t = Rc::clone(&net);
    vm.register_native("udp_send", Rc::new(move |ctx: &mut NativeCtx, a: &[Value]| {
        let sock = Args(a).word(0);
        let Some(addr) = read_string(ctx.heap, Args(a).handle(1)) else { return ret_unit(); };
        let data = Args(a).handle(2);
        if data.is_handle_none() { return ret_unit(); }
        let (slot, gen_) = data.as_handle();
        let bytes: Vec<u8> = ctx.heap.cell_data(slot, gen_)?.iter().map(|&w| w as u8).collect();
        t.borrow().send(sock, &addr, &bytes).ok_or_else(|| bad_sock(sock))?;
        ret_unit()
    }));
    let t = Rc::clone(&net);
    vm.register_native("udp_broadcast", Rc::new(move |_: &mut NativeCtx, a: &[Value]| {
        let id = Args(a).word(0);
        t.borrow().broadcast(id, Args(a).flag(1)).ok_or_else(|| bad_sock(id))?;
        ret_unit()
    }));
    let t = Rc::clone(&net);
    vm.register_native("udp_close", Rc::new(move |_: &mut NativeCtx, a: &[Value]| {
        t.borrow_mut().close(Args(a).word(0));
        ret_unit()
    }));
}

pub struct NetPlugin {
    pub net: Net,
}

impl NetPlugin {
    pub fn new(sockets: Box<dyn Sockets>) -> Self { Self { net: Rc::new(RefCell::new(sockets)) } }
    pub fn sweep(&self) { self.net.borrow_mut().sweep(); }
}

impl super::plugin::Plugin for NetPlugin {
    fn install(&self, vm: &mut VirtualMachine) {
        register_natives(vm, Rc::clone(&self.net));
    }
}
