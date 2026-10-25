#![cfg(feature = "gfx")]
use posara_core::{word32, word64};
use polka::{BytecodeChunk, Chunk, Module, OpCode, Register};

fn program(narrow: bool, writes: &[(u16, u64)]) -> Module {
    let mut chunk = BytecodeChunk { reg_count: 2, ..BytecodeChunk::default() };
    for &(port, value) in writes {
        let index = chunk.constants.len() as u16;
        chunk.constants.push(polka::Constant::from_le_bytes(u64::from(port).to_le_bytes()));
        chunk.constants.push(polka::Constant::from_le_bytes(value.to_le_bytes()));
        chunk.code.extend([OpCode::PushConst(Register(0), index), OpCode::PushConst(Register(1), index + 1), OpCode::Deo(Register(1), Register(0))]);
    }
    chunk.code.push(OpCode::Ret(Register(1)));
    Module { functions: vec![Chunk::Bytecode(chunk)], flags: if narrow { polka::CART_FLAG_INT32_SAFE } else { 0 }, ..Module::default() }
}

#[test]
fn narrow_deo_configures_dimensions_with_default_or_explicit_rgb565() {
    for explicit in [false, true] {
        let host = word32::Host::new(word32::Drivers::null());
        let mut vm = word32::VirtualMachine::new(); host.install(&mut vm);
        let mut writes = Vec::new();
        if explicit { writes.push((0x2002, 1)); }
        writes.push((0x2000, 2 | (3 << 16)));
        vm.run_module(&program(true, &writes)).unwrap();
        let fb = host.gfx.fb.borrow();
        assert_eq!((fb.w, fb.h, fb.format, fb.buf.len()), (2, 3, 1, 6));
    }
}

#[test]
fn narrow_deo_rejects_unsupported_format_without_configuring() {
    for format in [0, 2, 256, u32::MAX as u64] {
        let host = word32::Host::new(word32::Drivers::null());
        let mut vm = word32::VirtualMachine::new(); host.install(&mut vm);
        assert!(vm.run_module(&program(true, &[(0x2002, format), (0x2000, 2 | (3 << 16))])).is_err());
        assert!(host.gfx.fb.borrow().buf.is_empty());
    }
}

#[test]
fn wide_deo_keeps_the_existing_forty_bit_configuration() {
    let host = word64::Host::new(word64::Drivers::null());
    let mut vm = word64::VirtualMachine::new(); host.install(&mut vm);
    vm.run_module(&program(false, &[(0x2000, 2 | (3 << 16) | (1 << 32))])).unwrap();
    assert_eq!(host.gfx.fb.borrow().buf.len(), 6);
    assert!(vm.run_module(&program(false, &[(0x2002, 1)])).is_err());
}
