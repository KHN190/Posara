use std::collections::VecDeque;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use midir::{Ignore, MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};
use midir::os::unix::VirtualOutput;
use myriad::{Device, VirtualMachine};
use myriad::memory::Heap;
use polka::Value;

use crate::plugin::Plugin;
pub mod router;

use router::{self as midi_router, Dest, NodeKind, RoutePlan};

pub const MIDI_ID: u8 = 0x90;
pub const PORT_EVENT: u8 = 0x00;
pub const PORT_COUNT: u8 = 0x01;
pub const PORT_SEND: u8 = 0x02;

pub type MidiQueue = Arc<Mutex<VecDeque<u32>>>;
pub type MidiOut = Arc<Mutex<MidiOutputConnection>>;
type Outs = Arc<Mutex<Vec<Option<MidiOut>>>>;

struct MidiDevice {
    queue: MidiQueue,
    outs: Outs,           // dest index = declared out_ports order
    virt: Option<MidiOut>, // this cart's virtual source ("output jack")
}

fn send_one(out: &MidiOut, msg: &[u8]) -> Result<(), String> {
    let mut o = out.lock().map_err(|_| "MIDI: out poisoned")?;
    o.send(msg).map_err(|e| format!("MIDI send: {e}"))
}

impl Device for MidiDevice {
    fn read(&mut self, port: u8) -> Result<(Value, bool), String> {
        let mut q = self.queue.lock().map_err(|_| "MIDI: queue poisoned")?;
        match port {
            PORT_EVENT => Ok((Value::from_int(q.pop_front().unwrap_or(0) as i64), false)),
            PORT_COUNT => Ok((Value::from_int(q.len() as i64), false)),
            _ => Err(format!("MIDI: read port {:#04x} unsupported", port)),
        }
    }

    fn write(&mut self, port: u8, val: Value, _is_handle: bool, _heap: &mut Heap) -> Result<(), String> {
        match port {
            PORT_SEND => {
                let w = val.as_int();
                let status = (w & 0xFF) as u8;
                let d1 = ((w >> 8) & 0xFF) as u8;
                let d2 = ((w >> 16) & 0xFF) as u8;
                let msg: &[u8] = match status & 0xF0 {
                    0xC0 | 0xD0 => &[status, d1],
                    _ => &[status, d1, d2],
                };
                // The virtual source carries every send; wires decide who listens.
                if let Some(v) = &self.virt {
                    send_one(v, msg)?;
                }
                let outs = self.outs.lock().map_err(|_| "MIDI: outs poisoned")?;
                match midi_router::dest_of(w) {
                    Dest::Broadcast => {
                        for out in outs.iter().flatten() {
                            send_one(out, msg)?;
                        }
                        Ok(())
                    }
                    Dest::Port(i) => match outs.get(i).and_then(|o| o.as_ref()) {
                        Some(out) => send_one(out, msg),
                        None => Ok(()),   // pending or absent: drop
                    },
                }
            }
            _ => Err(format!("MIDI: write port {:#04x} unsupported", port)),
        }
    }
}

// `eager` only when midi.toml routes this cart: its virtual source must exist
// at startup for others to wire to. Otherwise ports open on first 0x90 access.
pub struct MidiPlugin {
    eager: Option<EagerMidi>,
}

struct EagerMidi {
    queue: MidiQueue,
    outs: Outs,
    virt: Option<MidiOut>,
    _ins: Vec<MidiInputConnection<()>>,
}

fn connect_v1() -> (MidiDevice, Vec<MidiInputConnection<()>>) {
    let queue: MidiQueue = Arc::new(Mutex::new(VecDeque::new()));
    let mut ins = Vec::new();
    if let Some((name, c)) = subscribe(&queue, "*", 0, false) {
        eprintln!("• MIDI in: {name}");
        ins.push(c);
    }
    let outs = match open_out("*") {
        Some((name, o)) => {
            eprintln!("• MIDI out: {name}");
            vec![Some(o)]
        }
        None => vec![],
    };
    (MidiDevice { queue, outs: Arc::new(Mutex::new(outs)), virt: None }, ins)
}

struct LazyMidiDevice {
    inner: Option<(MidiDevice, Vec<MidiInputConnection<()>>)>,
}

impl LazyMidiDevice {
    fn dev(&mut self) -> &mut MidiDevice {
        if self.inner.is_none() {
            self.inner = Some(connect_v1());
        }
        &mut self.inner.as_mut().unwrap().0
    }
}

impl Device for LazyMidiDevice {
    fn read(&mut self, port: u8) -> Result<(Value, bool), String> {
        self.dev().read(port)
    }

    fn write(&mut self, port: u8, val: Value, is_handle: bool, heap: &mut Heap) -> Result<(), String> {
        self.dev().write(port, val, is_handle, heap)
    }
}

// One wire still waiting for its far end. node = display name; exact = match
// the port name exactly (cart virtual sources) vs glob (hardware).
struct PendingIn { node: String, pat: String, exact: bool, src_tag: u8 }
struct PendingOut { slot: usize, node: String, glob: String }

fn subscribe(queue: &MidiQueue, pat: &str, src_tag: u8, exact: bool) -> Option<(String, MidiInputConnection<()>)> {
    let mut input = MidiInput::new("posara").ok()?;
    input.ignore(Ignore::SysexAndTime);
    let ports = input.ports();
    let port = ports.iter().find(|p| {
        let n = input.port_name(p).unwrap_or_default();
        if exact { n == pat } else { midi_router::glob_match(pat, &n) }
    })?;
    let name = input.port_name(port).unwrap_or_default();
    let q = Arc::clone(queue);
    let c = input.connect(port, "posara-in", move |_, msg, _| {
        let mut w = (src_tag as u32) << 24;
        for (i, &b) in msg.iter().take(3).enumerate() { w |= (b as u32) << (8 * i); }
        if let Ok(mut q) = q.lock() { q.push_back(w); }
    }, ()).ok()?;
    Some((name, c))
}

fn open_out(glob: &str) -> Option<(String, MidiOut)> {
    let output = MidiOutput::new("posara").ok()?;
    let ports = output.ports();
    let port = ports.iter().find(|p| {
        midi_router::glob_match(glob, &output.port_name(p).unwrap_or_default())
    })?;
    let name = output.port_name(port).unwrap_or_default();
    let c = output.connect(port, "posara-out").ok()?;
    Some((name, Arc::new(Mutex::new(c))))
}

// CoreMIDI can report an empty port list right after client creation; retry
// briefly before trusting it.
fn scan_in_names() -> Vec<String> {
    for _ in 0..3 {
        let names: Vec<String> = MidiInput::new("posara-scan").map(|i| {
            i.ports().iter().map(|p| i.port_name(p).unwrap_or_default()).collect()
        }).unwrap_or_default();
        if !names.is_empty() {
            return names;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Vec::new()
}

// Retry pending wires every 2s until everything is connected. Connections made
// here live in the thread, which parks forever once done.
fn spawn_reconnect(queue: MidiQueue, outs: Outs, mut p_in: Vec<PendingIn>, mut p_out: Vec<PendingOut>) {
    if p_in.is_empty() && p_out.is_empty() {
        return;
    }
    std::thread::spawn(move || {
        let mut held: Vec<MidiInputConnection<()>> = Vec::new();
        loop {
            std::thread::sleep(Duration::from_secs(2));
            p_in.retain(|p| match subscribe(&queue, &p.pat, p.src_tag, p.exact) {
                Some((name, c)) => {
                    eprintln!("• midi: connected ← {} ({name})", p.node);
                    held.push(c);
                    false
                }
                None => true,
            });
            p_out.retain(|p| match open_out(&p.glob) {
                Some((name, o)) => {
                    eprintln!("• midi: connected → {} ({name})", p.node);
                    if let Ok(mut outs) = outs.lock() {
                        if let Some(slot) = outs.get_mut(p.slot) { *slot = Some(o); }
                    }
                    false
                }
                None => true,
            });
            if p_in.is_empty() && p_out.is_empty() {
                loop { std::thread::park(); }   // keep `held` alive
            }
        }
    });
}

impl MidiPlugin {
    pub fn new() -> Self {
        Self { eager: None }
    }

    // midi.toml routing. `root` holds midi.toml; `entry` is the cart path
    // relative to root. Falls back to v1 when there is no config or the cart
    // is not declared. Unmatched wires retry in the background.
    pub fn new_routed(root: &Path, entry: &Path) -> Self {
        let Ok(text) = std::fs::read_to_string(root.join("midi.toml")) else {
            return Self::new();
        };
        let cfg = match midi_router::parse(&text) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("• {e}; default routing");
                return Self::new();
            }
        };
        let entry_rel = entry.strip_prefix(root).unwrap_or(entry).to_string_lossy().to_string();

        let plan: RoutePlan = midi_router::resolve(&cfg, &entry_rel, &scan_in_names());
        for warn in &plan.warnings {
            eprintln!("• midi: {warn}");
        }
        let Some(node_name) = plan.node_name.clone() else {
            return Self::new();
        };

        let glob_of = |node: &str| -> Option<String> {
            match cfg.nodes.iter().find(|nd| nd.name == node).map(|nd| &nd.kind) {
                Some(NodeKind::Port(g)) => Some(g.clone()),
                _ => None,
            }
        };

        let queue: MidiQueue = Arc::new(Mutex::new(VecDeque::new()));
        let mut ins = Vec::new();
        let mut pending_in = Vec::new();
        let mut src_tag: u8 = 0;
        let mut in_desc = Vec::new();
        for n in &plan.in_carts {
            match subscribe(&queue, n, src_tag, true) {
                Some((name, c)) => {
                    in_desc.push(format!("{n} ({name})"));
                    ins.push(c);
                }
                None => {
                    eprintln!("• midi: `{n}` not running, wire pending");
                    pending_in.push(PendingIn { node: n.clone(), pat: n.clone(), exact: true, src_tag });
                }
            }
            src_tag += 1;
        }
        for sel in &plan.in_ports {
            let Some(glob) = glob_of(&sel.node) else { continue };
            match subscribe(&queue, &glob, src_tag, false) {
                Some((name, c)) => {
                    in_desc.push(format!("{} ({name})", sel.node));
                    ins.push(c);
                }
                None => pending_in.push(PendingIn { node: sel.node.clone(), pat: glob, exact: false, src_tag }),
            }
            src_tag += 1;
        }

        let mut outs = Vec::new();
        let mut pending_out = Vec::new();
        let mut out_desc = Vec::new();
        for sel in &plan.out_ports {
            let Some(glob) = glob_of(&sel.node) else { continue };
            match open_out(&glob) {
                Some((name, o)) => {
                    out_desc.push(format!("{} ({name})", sel.node));
                    outs.push(Some(o));
                }
                None => {
                    pending_out.push(PendingOut { slot: outs.len(), node: sel.node.clone(), glob });
                    outs.push(None);
                }
            }
        }
        let outs: Outs = Arc::new(Mutex::new(outs));

        let virt = match MidiOutput::new("posara").map(|o| o.create_virtual(&node_name)) {
            Ok(Ok(c)) => Some(Arc::new(Mutex::new(c))),
            Ok(Err(e)) => {
                eprintln!("• midi: virtual source `{node_name}` failed: {e}");
                None
            }
            Err(e) => {
                eprintln!("• midi: virtual source `{node_name}` failed: {e}");
                None
            }
        };

        eprintln!(
            "• midi: {node_name} → [{}]  ← [{}]",
            out_desc.join(", "),
            in_desc.join(", "),
        );
        spawn_reconnect(Arc::clone(&queue), Arc::clone(&outs), pending_in, pending_out);
        Self { eager: Some(EagerMidi { queue, outs, virt, _ins: ins }) }
    }
}

impl Plugin for MidiPlugin {
    fn install(&self, vm: &mut VirtualMachine) {
        match &self.eager {
            Some(e) => vm.install_device(MIDI_ID, Box::new(MidiDevice {
                queue: Arc::clone(&e.queue),
                outs: Arc::clone(&e.outs),
                virt: e.virt.clone(),
            })),
            None => vm.install_device(MIDI_ID, Box::new(LazyMidiDevice { inner: None })),
        }
    }

    #[cfg(feature = "compiler")]
    fn register_fns(&self, _compiler: &mut abrase::compiler::Compiler) -> Result<(), String> {
        Ok(())
    }
}
