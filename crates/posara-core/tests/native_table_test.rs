use std::collections::BTreeSet;

use myriad::devices::{SystemDevice, SYSTEM_ID};
use myriad::VirtualMachine;
use posara_core::abi::NATIVES;
use posara_core::host::{Drivers, Host};
use posara_core::stepper::Stepper;

fn fixture(name: &str) -> polka::Module {
    let p = format!("{}/../posara/tests/expected/{name}.pk", env!("CARGO_MANIFEST_DIR"));
    polka::cartridge::read_pk(&std::fs::read(p).unwrap()).unwrap()
}

fn vm_for(host: &Host) -> VirtualMachine {
    let mut vm = VirtualMachine::new();
    vm.install_device(SYSTEM_ID, Box::new(SystemDevice::new()));
    host.install(&mut vm);
    vm
}

#[test]
fn pk_from_full_desktop_build_loads_with_any_capabilities() {
    let host = Host::new(Drivers::null());
    let mut step = Stepper::start(vm_for(&host), fixture("full-native-table")).unwrap_or_else(|e| panic!("{e}"));
    assert!(step.frame().unwrap());
}

#[test]
fn native_names_are_unique() {
    let names: BTreeSet<_> = NATIVES.iter().map(|s| s.name).collect();
    assert_eq!(names.len(), NATIVES.len());
}

#[test]
fn time_follows_the_virtual_clock_when_set() {
    let host = Host::new(Drivers::null());
    host.clock.set(Some(1234));
    assert_eq!(host.now_ms(), 1234);
    host.clock.set(None);
    assert_eq!(host.now_ms(), 0);
}

#[cfg(not(all(feature = "fs", feature = "net")))]
#[test]
fn stubbed_text_and_bytes_natives_return_empty_values() {
    let host = Host::new(Drivers::null());
    let mut step = Stepper::start(vm_for(&host), fixture("stub-returns")).unwrap_or_else(|e| panic!("{e}"));
    step.frame().unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(step.exit_code(), 7);
}

#[test]
fn a_wrong_typed_handle_reads_as_no_bytes() {
    let mut heap = myriad::Heap::new();
    let (slot, gen_) = heap.alloc(2);
    heap.st(slot, gen_, 0, u64::MAX, false).unwrap();
    assert_eq!(myriad::read_bytes(&heap, myriad::Value::from_handle(slot, gen_)), None);
}

mod argument_domains {
    use myriad::Value;
    use posara_core::abi::{Args, COORD_LIM, MAX_MS, OFFSET_LIM};
    use proptest::prelude::*;

    fn raw() -> impl Strategy<Value = Value> {
        prop_oneof![
            any::<i64>().prop_map(Value::from_int),
            any::<f64>().prop_map(Value::from_float),
            Just(Value::from_int(i64::MIN)), Just(Value::from_int(i64::MAX)), Just(Value::from_float(f64::NAN)),
        ]
    }

    proptest! {
        #[test]
        fn every_argument_kind_decodes_into_its_domain(v in raw()) {
            let a = Args(&[v]);
            prop_assert!((-COORD_LIM..=COORD_LIM).contains(&a.coord(0)));
            prop_assert!((-OFFSET_LIM..=OFFSET_LIM).contains(&a.offset(0)) && (-OFFSET_LIM..=OFFSET_LIM).contains(&a.degrees(0)));
            prop_assert!(a.index(0) <= u32::MAX as usize);
            prop_assert!(a.ms(0) as i64 <= MAX_MS);
            prop_assert!((0.0..=1.0).contains(&a.percent(0)) && (-1.0..=1.0).contains(&a.balance(0)));
            prop_assert!((-128..=128).contains(&a.semitones(0)));
            for x in [a.cents(0), a.hz(0), a.level(0), a.amount(0)] { prop_assert!(x.is_finite()); }
            prop_assert!(a.hz(0) >= 1.0 && a.amount(0) >= 0.0);
            prop_assert!(math_is_finite(a.semitones(0) as f32 + a.cents(0) / 100.0));
            prop_assert!((3..=9).contains(&a.within(0, 3, 9)));
            match a.count(0, 1000) {
                Ok(n) => prop_assert!(n <= 1000),
                Err(e) => prop_assert!(a.word(0) > 1000, "{e}"),
            }
        }
    }

    fn math_is_finite(semis: f32) -> bool { 2f32.powf(semis / 12.0).is_finite() }

    #[test]
    fn missing_arguments_read_as_zero_none_and_false() {
        let a = Args(&[]);
        assert_eq!((a.word(0), a.coord(3), a.flag(1)), (0, 0, false));
        assert!(a.handle(2).is_handle_none());
    }
}

#[test]
fn native_names_order_arguments_results_effects_and_capabilities_are_frozen() {
    let actual: String = NATIVES.iter().map(|s| {
        let args = s.params.iter().map(|p| format!("{p:?}")).collect::<Vec<_>>().join(", ");
        format!("{}({args}) -> {:?} {:?} {:?}\n", s.name, s.ret, s.eff, s.cap)
    }).collect();
    assert_eq!(actual, include_str!("../../posara/tests/contracts/natives.txt"),
        "Public API changed. Adding, removing or changing a native requires explicit user approval; do not regenerate this baseline.");
}

#[test]
fn only_the_approved_110_native_contracts_differ_from_101() {
    use std::collections::BTreeMap;
    let index=|source:&str|->BTreeMap<String,String>{source.lines().map(|line|(line.split('(').next().unwrap().into(),line.into())).collect()};
    let old=index(include_str!("../../posara/tests/contracts/natives-1.0.1.txt"));let new=index(include_str!("../../posara/tests/contracts/natives.txt"));
    let changed:Vec<_>=old.iter().filter(|(name,sig)|new.get(*name)!=Some(*sig)).map(|(name,_)|name.as_str()).collect();
    assert_eq!(changed,vec!["fs_read","fs_reads","fs_write","snd_seq","snd_track"]);
    assert!(new.keys().all(|name|old.contains_key(name)),"1.1 must not introduce unapproved natives");
}

#[test]
fn contract_snapshots_and_cart_output_oracles_are_unchanged() {
    let status = std::process::Command::new("python3")
        .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/../../scripts/check-contract.py"))
        .status().expect("python3 is required by the compatibility harness");
    assert!(status.success(), "Native snapshots or cart output oracles changed.");
}
