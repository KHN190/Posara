#![cfg(feature = "midi")]
use posara::plugins::midi::router::*;

const FULL: &str = r#"
[nodes]
seq     = { cart = "music/seq.abe" }
tracker = { cart = "music/tracker.abe" }
op1     = { port = "OP-1*" }

[[wires]]
from = "seq"
to   = ["op1", "tracker"]

[[wires]]
from = "tracker"
to   = ["op1"]
"#;

// ---- parse: expected ----

#[test]
fn parse_full_config() {
    let cfg = parse(FULL).unwrap();
    assert_eq!(cfg.nodes.len(), 3);
    assert_eq!(cfg.wires.len(), 2);
    assert_eq!(cfg.wires[0].from, "seq");
    assert_eq!(cfg.wires[0].to, vec!["op1", "tracker"]);
}

#[test]
fn parse_empty_is_empty_config() {
    let cfg = parse("").unwrap();
    assert!(cfg.nodes.is_empty());
    assert!(cfg.wires.is_empty());
}

#[test]
fn parse_empty_to_is_ok() {
    let cfg = parse(r#"
[nodes]
seq = { cart = "a.abe" }
[[wires]]
from = "seq"
to = []
"#).unwrap();
    assert_eq!(cfg.wires[0].to.len(), 0);
}

// ---- parse: unexpected ----

#[test]
fn parse_bad_toml_fails() {
    assert!(parse("[nodes\nbroken").is_err());
}

#[test]
fn parse_wire_from_unknown_node_fails() {
    let e = parse(r#"
[nodes]
seq = { cart = "a.abe" }
[[wires]]
from = "ghost"
to = ["seq"]
"#).unwrap_err();
    assert!(e.contains("ghost"), "error should name the node: {e}");
}

#[test]
fn parse_wire_to_unknown_node_fails() {
    let e = parse(r#"
[nodes]
seq = { cart = "a.abe" }
[[wires]]
from = "seq"
to = ["ghost"]
"#).unwrap_err();
    assert!(e.contains("ghost"));
}

#[test]
fn parse_node_needs_cart_or_port() {
    assert!(parse("[nodes]\nx = { }\n").is_err());
}

#[test]
fn parse_node_cart_and_port_both_fails() {
    assert!(parse(r#"
[nodes]
x = { cart = "a.abe", port = "B*" }
"#).is_err());
}

// ---- glob ----

#[test]
fn glob_exact_and_wildcard() {
    assert!(glob_match("OP-1", "OP-1"));
    assert!(!glob_match("OP-1", "OP-12"));
    assert!(glob_match("OP-1*", "OP-1 MIDI"));
    assert!(glob_match("*总线1", "IAC驱动程序总线1"));
    assert!(glob_match("IAC*总线*", "IAC驱动程序总线1"));
    assert!(glob_match("*", "anything"));
    assert!(!glob_match("OP-1*", "KORG"));
}

// ---- resolve: expected ----

fn ports() -> Vec<String> {
    vec!["IAC驱动程序总线1".into(), "OP-1 MIDI Out".into()]
}

#[test]
fn resolve_declared_cart() {
    let cfg = parse(FULL).unwrap();
    let plan = resolve(&cfg, "music/seq.abe", &ports());
    assert_eq!(plan.node_name.as_deref(), Some("seq"));
}

#[test]
fn resolve_out_port_matched() {
    let cfg = parse(FULL).unwrap();
    let plan = resolve(&cfg, "music/seq.abe", &ports());
    // seq → op1: matches "OP-1 MIDI Out" at index 1.
    assert!(plan.out_ports.iter().any(|o| o.node == "op1" && o.port == Some(1)));
}

#[test]
fn resolve_listens_to_upstream_cart() {
    let cfg = parse(FULL).unwrap();
    // seq → tracker, so tracker listens to virtual source "seq".
    let plan = resolve(&cfg, "music/tracker.abe", &ports());
    assert!(plan.in_carts.iter().any(|n| n == "seq"));
}

#[test]
fn resolve_no_self_listen_without_wire() {
    let cfg = parse(FULL).unwrap();
    let plan = resolve(&cfg, "music/seq.abe", &ports());
    assert!(plan.in_carts.is_empty());   // nothing wired into seq
}

#[test]
fn resolve_in_port_wired_to_me() {
    let cfg = parse(r#"
[nodes]
keys = { port = "OP-1*" }
trk  = { cart = "t.abe" }
[[wires]]
from = "keys"
to = ["trk"]
"#).unwrap();
    let plan = resolve(&cfg, "t.abe", &ports());
    assert!(plan.in_ports.iter().any(|o| o.node == "keys" && o.port == Some(1)));
}

// ---- resolve: unexpected ----

#[test]
fn resolve_undeclared_cart_falls_back() {
    let cfg = parse(FULL).unwrap();
    let plan = resolve(&cfg, "games/surf.abe", &ports());
    assert!(plan.node_name.is_none());
    assert!(!plan.warnings.is_empty());
}

#[test]
fn resolve_port_unmatched_is_pending() {
    let cfg = parse(r#"
[nodes]
seq = { cart = "s.abe" }
korg = { port = "KORG*" }
[[wires]]
from = "seq"
to = ["korg"]
"#).unwrap();
    let plan = resolve(&cfg, "s.abe", &ports());
    let o = plan.out_ports.iter().find(|o| o.node == "korg").unwrap();
    assert_eq!(o.port, None);            // pending
    assert!(plan.warnings.iter().any(|w| w.contains("korg")));
}

#[test]
fn resolve_self_wire_warns() {
    let cfg = parse(r#"
[nodes]
seq = { cart = "s.abe" }
[[wires]]
from = "seq"
to = ["seq"]
"#).unwrap();
    let plan = resolve(&cfg, "s.abe", &ports());
    assert!(plan.warnings.iter().any(|w| w.contains("seq")));
}

// ---- event packing ----

#[test]
fn pack_event_src_in_high_bits() {
    let ev = pack_event(0x92, 60, 100, 3);
    assert_eq!(ev % 256, 0x92);
    assert_eq!(ev / 256 % 256, 60);
    assert_eq!(ev / 65536 % 256, 100);
    assert_eq!(ev / 16777216, 3);
}

#[test]
fn dest_of_legacy_value_is_port_zero() {
    assert_eq!(dest_of(0x90 + 60 * 256 + 100 * 65536), Dest::Port(0));
}

#[test]
fn dest_of_broadcast() {
    let v = 0x90 + 60 * 256 + 100 * 65536 + 0xFF * 16777216;
    assert_eq!(dest_of(v), Dest::Broadcast);
}

#[test]
fn dest_of_explicit_port() {
    let v = 0x90 + 60 * 256 + 100 * 65536 + 2 * 16777216;
    assert_eq!(dest_of(v), Dest::Port(2));
}
