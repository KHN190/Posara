// midi.toml routing: pure logic, no midir. See designs/midi.md.

#[derive(Debug, Clone, PartialEq)]
pub enum NodeKind {
    Cart(String),
    Port(String),
}

#[derive(Debug, Clone)]
pub struct Node {
    pub name: String,
    pub kind: NodeKind,
}

#[derive(Debug, Clone)]
pub struct Wire {
    pub from: String,
    pub to: Vec<String>,
}

#[derive(Debug, Default)]
pub struct MidiConfig {
    pub nodes: Vec<Node>,
    pub wires: Vec<Wire>,
}

impl MidiConfig {
    fn node(&self, name: &str) -> Option<&Node> {
        self.nodes.iter().find(|n| n.name == name)
    }
}

pub fn parse(s: &str) -> Result<MidiConfig, String> {
    let doc: toml::Value = s.parse().map_err(|e| format!("midi.toml: {e}"))?;
    let mut cfg = MidiConfig::default();

    if let Some(nodes) = doc.get("nodes") {
        let table = nodes.as_table().ok_or("midi.toml: [nodes] must be a table")?;
        for (name, v) in table {
            let cart = v.get("cart").and_then(|c| c.as_str());
            let port = v.get("port").and_then(|p| p.as_str());
            let kind = match (cart, port) {
                (Some(c), None) => NodeKind::Cart(c.to_string()),
                (None, Some(p)) => NodeKind::Port(p.to_string()),
                (Some(_), Some(_)) => {
                    return Err(format!("midi.toml: node `{name}` has both cart and port"))
                }
                (None, None) => {
                    return Err(format!("midi.toml: node `{name}` needs cart or port"))
                }
            };
            cfg.nodes.push(Node { name: name.clone(), kind });
        }
    }

    if let Some(wires) = doc.get("wires") {
        let arr = wires.as_array().ok_or("midi.toml: [[wires]] must be an array")?;
        for w in arr {
            let from = w.get("from").and_then(|f| f.as_str())
                .ok_or("midi.toml: wire needs `from`")?.to_string();
            let to: Vec<String> = w.get("to").and_then(|t| t.as_array())
                .ok_or("midi.toml: wire needs `to` array")?
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
            if cfg.node(&from).is_none() {
                return Err(format!("midi.toml: wire from unknown node `{from}`"));
            }
            for t in &to {
                if cfg.node(t).is_none() {
                    return Err(format!("midi.toml: wire to unknown node `{t}`"));
                }
            }
            cfg.wires.push(Wire { from, to });
        }
    }
    Ok(cfg)
}

// `*` wildcard match, anywhere in the pattern.
pub fn glob_match(pat: &str, s: &str) -> bool {
    let parts: Vec<&str> = pat.split('*').collect();
    if parts.len() == 1 {
        return pat == s;
    }
    let mut rest = s;
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if i == 0 {
            match rest.strip_prefix(part) {
                Some(r) => rest = r,
                None => return false,
            }
        } else if i == parts.len() - 1 {
            return rest.ends_with(part);
        } else {
            match rest.find(part) {
                Some(at) => rest = &rest[at + part.len()..],
                None => return false,
            }
        }
    }
    true
}

#[derive(Debug, PartialEq)]
pub struct PortSel {
    pub node: String,
    pub port: Option<usize>,   // None = pending (glob matched nothing yet)
}

#[derive(Debug, Default)]
pub struct RoutePlan {
    pub node_name: Option<String>,   // None = cart undeclared, default mode
    pub out_ports: Vec<PortSel>,     // port nodes this cart sends to
    pub in_ports: Vec<PortSel>,      // port nodes wired into this cart
    pub in_carts: Vec<String>,       // upstream cart nodes (their virtual sources)
    pub warnings: Vec<String>,
}

pub fn resolve(cfg: &MidiConfig, entry: &str, ports: &[String]) -> RoutePlan {
    let mut plan = RoutePlan::default();

    let me = cfg.nodes.iter().find(|n| matches!(&n.kind, NodeKind::Cart(c) if c == entry));
    let Some(me) = me else {
        if !cfg.nodes.is_empty() {
            plan.warnings.push(format!("cart `{entry}` not declared in midi.toml; default routing"));
        }
        return plan;
    };
    plan.node_name = Some(me.name.clone());

    let find_port = |glob: &str| ports.iter().position(|p| glob_match(glob, p));

    for w in &cfg.wires {
        if w.from == me.name {
            for t in &w.to {
                if *t == me.name {
                    plan.warnings.push(format!("wire `{}` → itself", me.name));
                    continue;
                }
                if let Some(NodeKind::Port(glob)) = cfg.node(t).map(|n| &n.kind) {
                    let port = find_port(glob);
                    if port.is_none() {
                        plan.warnings.push(format!("`{t}` (\"{glob}\") not found, wire pending"));
                    }
                    plan.out_ports.push(PortSel { node: t.clone(), port });
                }
                // cart targets subscribe to our virtual source; nothing to do here.
            }
        } else if w.to.iter().any(|t| t == &me.name) {
            match &cfg.node(&w.from).map(|n| n.kind.clone()) {
                Some(NodeKind::Port(glob)) => {
                    let port = find_port(glob);
                    if port.is_none() {
                        plan.warnings.push(format!("`{}` (\"{glob}\") not found, wire pending", w.from));
                    }
                    plan.in_ports.push(PortSel { node: w.from.clone(), port });
                }
                Some(NodeKind::Cart(_)) => plan.in_carts.push(w.from.clone()),
                None => {}
            }
        }
    }
    plan
}

// ev = status + d1·2^8 + d2·2^16 + src_port·2^24; low 24 bits match the v1 format.
pub fn pack_event(status: u8, d1: u8, d2: u8, src_port: u8) -> i64 {
    status as i64 + ((d1 as i64) << 8) + ((d2 as i64) << 16) + ((src_port as i64) << 24)
}

#[derive(Debug, PartialEq)]
pub enum Dest {
    Port(usize),
    Broadcast,
}

// Send value bits 24+: 0xFF = broadcast, otherwise a port index.
// Legacy carts leave them zero, which lands on port 0 — the v1 behavior.
pub fn dest_of(v: i64) -> Dest {
    match (v >> 24) & 0xFF {
        0xFF => Dest::Broadcast,
        p => Dest::Port(p as usize),
    }
}
