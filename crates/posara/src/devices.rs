// Single owner for device ids and the derived 16-bit device+port addresses.
// Plugins install under these ids; the lint's known-device set derives from the
// same list so it can never silently diverge from what is actually installed.

pub const SCREEN: u8 = 0x20;
pub const CONTROLLER: u8 = 0x80;
pub const MIDI: u8 = 0x90;

// myriad-provided devices (console, system clock/rng).
pub const CONSOLE: u8 = 0x00;
pub const SYSTEM: u8 = 0x10;
pub const SYSTEM_E0: u8 = 0xE0;
pub const SYSTEM_E1: u8 = 0xE1;
pub const SYSTEM_E2: u8 = 0xE2;

pub const KNOWN_IDS: &[u8] =
    &[CONSOLE, SYSTEM, SCREEN, CONTROLLER, MIDI, SYSTEM_E0, SYSTEM_E1, SYSTEM_E2];

// 16-bit address a cart writes: high byte = device id, low byte = port.
pub const fn port(id: u8, port: u8) -> u64 { ((id as u64) << 8) | port as u64 }
