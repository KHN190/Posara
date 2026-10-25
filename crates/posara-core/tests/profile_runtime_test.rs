use posara_core::{word32, word64};
use polka::{BytecodeChunk, Chunk, Module, OpCode, Register, CART_FLAG_INT32_SAFE};

fn module(narrow: bool) -> Module {
    Module {
        functions: vec![Chunk::Bytecode(BytecodeChunk {
            code: vec![OpCode::PushConst(Register(0), 0), OpCode::Ret(Register(0))], reg_count: 1,
            constants: vec![polka::Constant::from_le_bytes([0; 8])],
            ..BytecodeChunk::default()
        })],
        flags: if narrow { CART_FLAG_INT32_SAFE } else { 0 },
        ..Module::default()
    }
}

#[test]
fn native_profiles_have_real_distinct_abis() {
    assert_eq!(core::mem::size_of::<word32::Value>(), 4);
    assert_eq!(core::mem::size_of::<word64::Value>(), 8);
    let host = word32::Host::new(word32::Drivers::null());
    let mut vm = word32::VirtualMachine::new();
    host.install(&mut vm);
    assert_eq!(vm.run_module(&module(true)).unwrap().raw(), 0);
    assert!(vm.run_module(&module(false)).is_err());
    let host = word64::Host::new(word64::Drivers::null());
    let mut vm = word64::VirtualMachine::new();
    host.install(&mut vm);
    assert_eq!(vm.run_module(&module(false)).unwrap().raw(), 0);
    assert!(vm.run_module(&module(true)).is_err());
}

#[test]
fn random_float_mapping_never_reaches_one() {
    for bits in [0, 1, 0x7fff_ffff, 0xffff_ff00, u32::MAX] {
        let small: f32 = word32::random_float(bits);
        let large: f64 = word64::random_float(bits);
        assert!((0.0..1.0).contains(&small));
        assert!((0.0..1.0).contains(&large));
    }
}

#[test]
fn automatic_stepper_selects_and_keeps_the_cartridge_profile() {
    for narrow in [false, true] {
        let mut stepper = posara_core::RuntimeStepper::start(module(narrow)).unwrap();
        assert_eq!(stepper.profile(), if narrow { myriad::RuntimeProfile::Word32 } else { myriad::RuntimeProfile::Word64 });
        assert!(!stepper.frame().unwrap());
        assert!(!stepper.frame().unwrap());
    }
}

#[test]
fn typed_host_load_rejects_wrong_profile_before_changing_resources() {
    let host = word32::Host::new(word32::Drivers::null());
    *host.rng.borrow_mut() = 17;
    assert!(host.load(&module(false)).is_err());
    assert_eq!(*host.rng.borrow(), 17);
    assert!(host.load(&module(true)).is_ok());
    let host = word64::Host::new(word64::Drivers::null());
    assert!(host.load(&module(true)).is_err());
    assert!(host.load(&module(false)).is_ok());
}

#[test]
fn malformed_narrow_constant_is_rejected_before_prepare() {
    let mut invalid = module(true);
    let Chunk::Bytecode(chunk) = &mut invalid.functions[0] else { unreachable!() };
    chunk.constants[0] = polka::Constant::from_le_bytes((1u64 << 40).to_le_bytes());
    let mut prepared = false;
    assert!(posara_core::RuntimeStepper::start_with(invalid, |_| prepared = true).is_err());
    assert!(!prepared);
}

#[test]
fn narrow_offset_keeps_the_full_representable_input_domain() {
    let values = [word32::Value::from_int(i32::MIN), word32::Value::from_int(i32::MAX)];
    let args = word32::abi::Args(&values);
    assert_eq!(args.offset(0), i32::MIN);
    assert_eq!(args.offset(1), i32::MAX);
}

#[cfg(feature = "fs")]
#[test]
fn narrow_storage_seek_checks_overflow_without_changing_the_cursor() {
    use word32::fs::storage::{MemStorage, Storage, F_CREATE, F_WRITE};
    let storage = MemStorage::new();
    let fd = storage.open("sample", F_CREATE | F_WRITE);
    assert!(fd >= 0);
    assert_eq!(storage.seek(fd, 0, i32::MAX), i32::MAX);
    assert_eq!(storage.seek(fd, 1, 1), -1);
    assert_eq!(storage.seek(fd, 1, 0), i32::MAX);
    assert_eq!(storage.write(fd, &[1]), None);
    assert_eq!(storage.seek(fd, 1, 0), i32::MAX);
    assert_eq!(storage.read_file("sample"), Some(Vec::new()));
    assert_eq!(storage.write(fd, &[]), Some(0));
    assert_eq!(storage.seek(fd, 1, 0), i32::MAX);
    assert_eq!(storage.seek(fd, 1, i32::MIN), -1);
    assert_eq!(storage.seek(fd, 1, 0), i32::MAX);
}

#[test]
fn narrow_clock_and_now_native_carry_one_word() {
    struct FixedClock;
    impl word32::drivers::Clock for FixedClock { fn now_ms(&self) -> u32 { u32::MAX } }
    let mut drivers = word32::Drivers::null();
    drivers.clock = std::rc::Rc::new(FixedClock);
    let host = word32::Host::new(drivers);
    let mut vm = word32::VirtualMachine::new();
    host.install(&mut vm);
    let module = Module {
        functions: vec![
            Chunk::Bytecode(BytecodeChunk { code: vec![OpCode::Call(Register(0), 1), OpCode::Ret(Register(0))], reg_count: 1, ..BytecodeChunk::default() }),
            Chunk::Native(polka::NativeChunk { name: "now".into(), param_count: 0 }),
        ],
        flags: CART_FLAG_INT32_SAFE,
        ..Module::default()
    };
    assert_eq!(vm.run_module(&module).unwrap().raw(), u32::MAX);
}

#[cfg(feature = "net")]
#[test]
fn mismatched_host_load_does_not_sweep_live_sockets() {
    use std::{cell::Cell, rc::Rc};
    struct Sockets(Rc<Cell<usize>>);
    impl word32::net::Sockets for Sockets {
        fn bind(&mut self, _: u16) -> i32 { -1 }
        fn addr(&self, _: i32) -> Option<String> { None }
        fn recv(&mut self, _: i32) -> Option<usize> { None }
        fn recv_buf(&self) -> &[u8] { &[] }
        fn from(&self, _: i32) -> Option<String> { None }
        fn send(&self, _: i32, _: &str, _: &[u8]) -> Option<()> { None }
        fn broadcast(&self, _: i32, _: bool) -> Option<()> { None }
        fn close(&mut self, _: i32) {}
        fn live(&self) -> usize { 1 }
        fn sweep(&mut self) { self.0.set(self.0.get() + 1); }
    }
    let sweeps = Rc::new(Cell::new(0));
    let mut drivers = word32::Drivers::null();
    drivers.sockets = Box::new(Sockets(Rc::clone(&sweeps)));
    let host = word32::Host::new(drivers);
    assert!(host.load(&module(false)).is_err());
    assert_eq!(sweeps.get(), 0);
    assert!(host.load(&module(true)).is_ok());
    assert_eq!(sweeps.get(), 1);
}
