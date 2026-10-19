pub use posara_core::net::*;

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use std::net::UdpSocket;
    use super::{MAX_SOCKETS, Sockets};

    const MAX_DGRAM: usize = 65507;
    // id = generation << SLOT_BITS | slot. A reused slot bumps its generation, so
    // an id held across close→rebind fails the gen check instead of silently
    // aliasing the new socket. -1 (all ones) decodes to an out-of-range slot.
    const SLOT_BITS: u32 = 20;

    fn pack(slot: usize, generation: u32) -> i64 { ((generation as i64) << SLOT_BITS) | slot as i64 }
    fn unpack(id: i64) -> Option<(usize, u32)> {
        if id < 0 { return None; }
        Some(((id & ((1 << SLOT_BITS) - 1)) as usize, (id >> SLOT_BITS) as u32))
    }

    struct Entry {
        sock: UdpSocket,
        last_from: String,
    }

    struct Slot {
        generation: u32,
        entry: Option<Entry>,
    }

    pub struct SocketTable {
        slots: Vec<Slot>,
        scratch: Vec<u8>,          // reused recv buffer — no 64KB alloc per poll
        got: usize,                // bytes of the last recv, valid in scratch[..got]
    }

    impl SocketTable {
        pub fn new() -> Self { Self { slots: Vec::new(), scratch: vec![0u8; MAX_DGRAM], got: 0 } }

        fn get(&self, id: i64) -> Option<&Entry> {
            let (i, g) = unpack(id)?;
            let s = self.slots.get(i)?;
            if s.generation == g { s.entry.as_ref() } else { None }
        }

        #[cfg(test)]
        pub fn broadcast_get(&self, id: i64) -> bool {
            self.get(id).and_then(|e| e.sock.broadcast().ok()).unwrap_or(false)
        }
    }

    impl Sockets for SocketTable {
        fn live(&self) -> usize { self.slots.iter().filter(|s| s.entry.is_some()).count() }

        fn bind(&mut self, port: u16) -> i64 {
            if self.live() >= MAX_SOCKETS { return -1; }
            let Ok(sock) = UdpSocket::bind(("0.0.0.0", port)) else { return -1; };
            if sock.set_nonblocking(true).is_err() { return -1; }
            let entry = Entry { sock, last_from: String::new() };
            match self.slots.iter().position(|s| s.entry.is_none()) {
                Some(i) => { self.slots[i].generation += 1; self.slots[i].entry = Some(entry); pack(i, self.slots[i].generation) }
                None => { self.slots.push(Slot { generation: 1, entry: Some(entry) }); pack(self.slots.len() - 1, 1) }
            }
        }

        // bound address; host is the 0.0.0.0 wildcard, so only the port is
        // meaningful (learn your ephemeral bind(0) port). Peers learn your
        // routable address from a packet's source via from(), not from here.
        fn addr(&self, id: i64) -> Option<String> {
            let e = self.get(id)?;
            Some(e.sock.local_addr().map(|a| a.to_string()).unwrap_or_default())
        }

        fn recv(&mut self, id: i64) -> Option<usize> {
            self.got = 0;
            let (i, g) = unpack(id)?;
            let scratch = &mut self.scratch;                 // disjoint field borrow
            let s = self.slots.get_mut(i)?;
            if s.generation != g { return None; }
            let e = s.entry.as_mut()?;
            if let Ok((n, src)) = e.sock.recv_from(scratch) {
                e.last_from = src.to_string();
                self.got = n;
            }
            Some(self.got)
        }

        fn recv_buf(&self) -> &[u8] { &self.scratch[..self.got] }

        fn from(&self, id: i64) -> Option<String> {
            self.get(id).map(|e| e.last_from.clone())
        }

        fn send(&self, id: i64, addr: &str, data: &[u8]) -> Option<()> {
            let e = self.get(id)?;
            if !data.is_empty() { let _ = e.sock.send_to(data, addr); }   // valid socket; drops stay silent (UDP)
            Some(())
        }

        fn broadcast(&self, id: i64, on: bool) -> Option<()> {
            let e = self.get(id)?;
            let _ = e.sock.set_broadcast(on);
            Some(())
        }

        fn close(&mut self, id: i64) {
            if let Some((i, g)) = unpack(id) {
                if let Some(s) = self.slots.get_mut(i) {
                    if s.generation == g { s.entry = None; }
                }
            }
        }

        fn sweep(&mut self) { for s in &mut self.slots { s.entry = None; } }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use imp::SocketTable;
#[cfg(target_arch = "wasm32")]
pub use posara_core::net::NullSockets as SocketTable;


#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    type Net = Rc<RefCell<SocketTable>>;

    fn addr_of(t: &Net, id: i64) -> String { t.borrow().addr(id).unwrap_or_default() }
    fn from_of(t: &Net, id: i64) -> String { t.borrow().from(id).unwrap_or_default() }

    // loopback oracle: two sockets, A send → B recv, bytes equal + from = A's real port.
    #[test]
    fn loopback_roundtrip() {
        let t: Net = Rc::new(RefCell::new(SocketTable::new()));
        let a = t.borrow_mut().bind(0);
        let b = t.borrow_mut().bind(0);
        assert!(a >= 0 && b >= 0);
        let a_addr = addr_of(&t, a);
        let b_addr = addr_of(&t, b);
        assert!(a_addr.starts_with("0.0.0.0:") && !a_addr.ends_with(":0"));

        let msg = b"/freq\0\0\0,f\0\0\x43\xdc\x00\x00";
        // send to loopback explicitly; bound to 0.0.0.0 receives it.
        let dst = b_addr.replace("0.0.0.0", "127.0.0.1");
        t.borrow().send(a, &dst, msg).unwrap();

        let got = recv_until(&t, b);
        assert_eq!(got, msg);
        let from = from_of(&t, b);
        let a_port = a_addr.rsplit(':').next().unwrap();
        assert!(from.ends_with(a_port), "from {from} should carry A port {a_port}");
    }

    #[test]
    fn recv_empty_nonblocking() {
        let t: Net = Rc::new(RefCell::new(SocketTable::new()));
        let s = t.borrow_mut().bind(0);
        assert_eq!(t.borrow_mut().recv(s), Some(0));   // valid socket, no packet → Some(0), no block
    }

    #[test]
    fn bytes_binary_intact() {
        let t: Net = Rc::new(RefCell::new(SocketTable::new()));
        let a = t.borrow_mut().bind(0);
        let b = t.borrow_mut().bind(0);
        let dst = addr_of(&t, b).replace("0.0.0.0", "127.0.0.1");
        let payload: Vec<u8> = (0..=255u8).chain(std::iter::once(0)).collect();
        t.borrow().send(a, &dst, &payload).unwrap();
        assert_eq!(recv_until(&t, b), payload);
    }

    #[test]
    fn send_empty_rejected() {
        let t: Net = Rc::new(RefCell::new(SocketTable::new()));
        let a = t.borrow_mut().bind(0);
        let b = t.borrow_mut().bind(0);
        let dst = addr_of(&t, b).replace("0.0.0.0", "127.0.0.1");
        t.borrow().send(a, &dst, b"").unwrap();   // valid socket, empty data: no datagram leaves
        assert!(recv_within(&t, b, 150).is_none());   // loopback is sub-ms; 150 is ample
    }

    #[test]
    fn soft_cap_rejects() {
        let t: Net = Rc::new(RefCell::new(SocketTable::new()));
        for _ in 0..MAX_SOCKETS { assert!(t.borrow_mut().bind(0) >= 0); }
        assert_eq!(t.borrow_mut().bind(0), -1);   // cap+1 → error, no panic
    }

    // 2000 bind/close cycles: if close leaked the fd, this blows past the OS
    // fd ulimit and bind starts returning -1. All-succeed proves fds are freed.
    #[test]
    fn close_no_fd_leak() {
        let t: Net = Rc::new(RefCell::new(SocketTable::new()));
        for _ in 0..2000 {
            let s = t.borrow_mut().bind(0);
            assert!(s >= 0, "bind failed mid-cycle → fd leak");
            t.borrow_mut().close(s);
        }
        assert_eq!(t.borrow().live(), 0);
    }

    // ABA guard: an id held across close→rebind (which reuses the slot) is stale
    // and inert, never silently aliasing the new socket.
    #[test]
    fn stale_id_rejected_after_close() {
        let t: Net = Rc::new(RefCell::new(SocketTable::new()));
        let a = t.borrow_mut().bind(0);
        t.borrow_mut().close(a);
        let b = t.borrow_mut().bind(0);          // reuses the slot, bumps generation
        assert_ne!(a, b, "rebind must not reissue the same id");
        assert_eq!(t.borrow().addr(a), None);    // stale id: rejected, not aliased
        assert_eq!(t.borrow_mut().recv(a), None);
        assert!(!addr_of(&t, b).is_empty());     // new id: live
    }

    // discovery: B learns A's routable source from the packet (from()), not from
    // A's wildcard bind addr, and replies there — the cart↔cart primitive.
    #[test]
    fn discovery_reply_via_from() {
        let t: Net = Rc::new(RefCell::new(SocketTable::new()));
        let a = t.borrow_mut().bind(0);
        let b = t.borrow_mut().bind(0);
        let b_dst = addr_of(&t, b).replace("0.0.0.0", "127.0.0.1");
        t.borrow().send(a, &b_dst, b"who").unwrap();
        assert_eq!(recv_until(&t, b), b"who");
        let a_from = from_of(&t, b);
        assert!(a_from.starts_with("127.0.0.1:"), "from must be A's routable source, got {a_from}");
        t.borrow().send(b, &a_from, b"ack").unwrap();
        assert_eq!(recv_until(&t, a), b"ack");    // reply reached A via learned addr
    }

    #[test]
    fn sweep_closes_all() {
        let t: Net = Rc::new(RefCell::new(SocketTable::new()));
        for _ in 0..MAX_SOCKETS { t.borrow_mut().bind(0); }
        t.borrow_mut().sweep();
        assert_eq!(t.borrow().live(), 0);
    }

    #[test]
    fn broadcast_sets_flag() {
        let t: Net = Rc::new(RefCell::new(SocketTable::new()));
        let s = t.borrow_mut().bind(0);
        assert!(!t.borrow().broadcast_get(s));      // default off (no LAN spray)
        t.borrow().broadcast(s, true).unwrap();
        assert!(t.borrow().broadcast_get(s));       // SO_BROADCAST actually set
        t.borrow().broadcast(s, false).unwrap();
        assert!(!t.borrow().broadcast_get(s));
    }

    // timeout-guard: recv is nonblocking, so a bounded poll loop can never hang.
    fn recv_until(t: &Net, id: i64) -> Vec<u8> {
        recv_within(t, id, 1000).unwrap_or_default()
    }
    fn recv_within(t: &Net, id: i64, iters: u32) -> Option<Vec<u8>> {
        for _ in 0..iters {
            let mut tb = t.borrow_mut();
            if tb.recv(id).unwrap_or(0) > 0 { return Some(tb.recv_buf().to_vec()); }
            drop(tb);
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        None
    }
}
