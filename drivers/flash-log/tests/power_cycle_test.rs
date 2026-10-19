use std::convert::Infallible;

use embedded_storage::nor_flash::{ErrorType, NorFlash, ReadNorFlash};
use posara_core::fs::{MemStorage, Storage};
use posara_flash_log::FlashLog;

const BASE: u32 = 8192;
const SECTOR: u32 = 4096;
const LOG: u32 = 2 * SECTOR;

type Nor = Mock<1>;

struct Mock<const W: usize> { bits: Vec<u8>, erases: usize, writes: usize, power: Option<usize> }

impl<const W: usize> Mock<W> {
    fn new() -> Self { Self { bits: vec![0xFF; 4 * SECTOR as usize], erases: 0, writes: 0, power: None } }

    fn spend(&mut self, n: usize) -> usize {
        match &mut self.power {
            None => n,
            Some(left) => { let k = n.min(*left); *left -= k; k }
        }
    }
}
impl<const W: usize> ErrorType for Mock<W> { type Error = Infallible; }
impl<const W: usize> ReadNorFlash for Mock<W> {
    const READ_SIZE: usize = 1;
    fn read(&mut self, off: u32, buf: &mut [u8]) -> Result<(), Infallible> {
        buf.copy_from_slice(&self.bits[off as usize..off as usize + buf.len()]);
        Ok(())
    }
    fn capacity(&self) -> usize { self.bits.len() }
}
impl<const W: usize> NorFlash for Mock<W> {
    const WRITE_SIZE: usize = W;
    const ERASE_SIZE: usize = SECTOR as usize;
    fn erase(&mut self, from: u32, to: u32) -> Result<(), Infallible> {
        assert!(from % SECTOR == 0 && to % SECTOR == 0);
        assert!(from >= BASE && to <= BASE + LOG, "erase outside the log");
        if self.spend(1) == 1 {
            self.bits[from as usize..to as usize].fill(0xFF);
            self.erases += 1;
        }
        Ok(())
    }
    fn write(&mut self, off: u32, data: &[u8]) -> Result<(), Infallible> {
        assert!(off >= BASE && off as usize + data.len() <= (BASE + LOG) as usize, "write outside the log");
        assert!(off as usize % W == 0 && data.len() % W == 0, "unaligned write {off}+{}", data.len());
        let n = self.spend(data.len());
        for (b, d) in self.bits[off as usize..].iter_mut().zip(&data[..n]) { *b &= d; }
        self.writes += 1;
        Ok(())
    }
}

fn files(pairs: &[(&str, &[u8])]) -> MemStorage {
    let m = MemStorage::new();
    for (n, d) in pairs { m.add_file(n, d.to_vec()).unwrap(); }
    m
}

fn boot<const W: usize>(nor: Mock<W>) -> (FlashLog<Mock<W>>, MemStorage) {
    let mut log = FlashLog::new(nor, BASE, LOG);
    let mem = MemStorage::new();
    log.restore(&mem).unwrap();
    (log, mem)
}

#[test]
fn blank_flash_restores_no_files() {
    let (_, mem) = boot(Nor::new());
    assert!(mem.file_names().is_empty());
}

#[test]
fn saved_files_come_back_after_power_off() {
    let (mut log, _) = boot(Nor::new());
    assert!(log.store(&files(&[("synth.sav", &[1, 171, 4, 5, 6, 90, 0, 7, 1]), ("b", &[])])).unwrap());
    let (_, mem) = boot(log.release());
    assert_eq!(mem.file_names(), ["b", "synth.sav"]);
    assert_eq!(mem.read_file("synth.sav").unwrap(), [1, 171, 4, 5, 6, 90, 0, 7, 1]);
    assert_eq!(mem.read_file("b").unwrap(), Vec::<u8>::new());
}

#[test]
fn the_latest_save_wins() {
    let (mut log, _) = boot(Nor::new());
    log.store(&files(&[("s", &[1])])).unwrap();
    log.store(&files(&[("s", &[2])])).unwrap();
    let (_, mem) = boot(log.release());
    assert_eq!(mem.read_file("s").unwrap(), [2]);
}

#[test]
fn unchanged_files_leave_flash_untouched() {
    let (mut log, _) = boot(Nor::new());
    log.store(&files(&[("s", &[1])])).unwrap();
    let (mut log, mem) = boot(log.release());
    let before = (log.flash().writes, log.flash().erases);
    assert!(!log.store(&mem).unwrap());
    assert_eq!((log.flash().writes, log.flash().erases), before);
}

#[test]
fn saves_append_until_the_sector_fills_then_erase_once() {
    let (mut log, _) = boot(Nor::new());
    let mut n = 0u32;
    while log.flash().erases == 0 {
        n += 1;
        log.store(&files(&[("synth.sav", &n.to_le_bytes())])).unwrap();
    }
    assert!(n > 100, "{n} saves per sector");
    assert_eq!(log.flash().erases, 1);
    let (_, mem) = boot(log.release());
    assert_eq!(mem.read_file("synth.sav").unwrap(), n.to_le_bytes());
}

#[test]
fn a_torn_save_falls_back_to_the_one_before() {
    let (mut log, _) = boot(Nor::new());
    log.store(&files(&[("s", &[1])])).unwrap();
    log.store(&files(&[("s", &[2])])).unwrap();
    let mut nor = log.release();
    let last = (BASE..BASE + LOG).rev().find(|&a| nor.bits[a as usize] != 0xFF).unwrap();
    nor.bits[last as usize] = 0xFF;
    let (mut log, mem) = boot(nor);
    assert_eq!(mem.read_file("s").unwrap(), [1]);
    log.store(&files(&[("s", &[3])])).unwrap();
    let (_, mem) = boot(log.release());
    assert_eq!(mem.read_file("s").unwrap(), [3]);
}

#[test]
fn files_too_big_for_the_sector_are_refused() {
    let (mut log, _) = boot(Nor::new());
    assert!(log.store(&files(&[("big", &[0; SECTOR as usize])])).is_err());
    assert_eq!(log.flash().writes, 0);
}

use std::collections::BTreeMap;

use proptest::prelude::*;

type Files = BTreeMap<String, Vec<u8>>;

fn stored(f: &Files) -> MemStorage {
    let m = MemStorage::new();
    for (n, d) in f { m.add_file(n, d.clone()).unwrap(); }
    m
}

fn read_back(m: &MemStorage) -> Files {
    m.file_names().into_iter().map(|n| { let d = m.read_file(&n).unwrap(); (n, d) }).collect()
}

fn fits<const W: usize>(f: &Files) -> bool {
    let body: usize = 1 + f.iter().map(|(n, d)| 1 + n.len() + 2 + d.len()).sum::<usize>();
    (12 + body).next_multiple_of(W) <= SECTOR as usize
}

fn file_sets() -> impl Strategy<Value = Files> {
    prop::collection::btree_map("[a-z][a-z0-9_.]{0,11}", prop::collection::vec(any::<u8>(), 0..1200), 0..5)
}

#[derive(Clone, Debug)]
enum Op { Save(Files), Reboot }

fn ops() -> impl Strategy<Value = Vec<Op>> {
    prop::collection::vec(prop_oneof![3 => file_sets().prop_map(Op::Save), 1 => Just(Op::Reboot)], 0..40)
}

fn replay<const W: usize>(ops: Vec<Op>) -> Result<(), TestCaseError> {
    let (mut log, mem) = boot(Mock::<W>::new());
    let mut last = read_back(&mem);
    prop_assert!(last.is_empty());
    let mut saves = 0;
    for op in ops {
        match op {
            Op::Save(f) => {
                let r = log.store(&stored(&f));
                if fits::<W>(&f) {
                    prop_assert_eq!(r.unwrap(), f != last);
                    if f != last { saves += 1; }
                    last = f;
                } else {
                    prop_assert!(r.is_err());
                }
            }
            Op::Reboot => {
                let (l, mem) = boot(log.release());
                prop_assert_eq!(read_back(&mem), last.clone());
                log = l;
            }
        }
        prop_assert!(log.flash().erases <= saves);
    }
    let (_, mem) = boot(log.release());
    prop_assert_eq!(read_back(&mem), last);
    Ok(())
}

fn cut<const W: usize>(before: Files, during: Files, after: Files, power: usize) -> Result<(), TestCaseError> {
    prop_assume!(fits::<W>(&before) && fits::<W>(&during) && fits::<W>(&after));
    prop_assume!(before != during && during != after);
    let (mut log, _) = boot(Mock::<W>::new());
    for _ in 0..3 {
        log.store(&stored(&before)).unwrap();
        log.store(&stored(&Files::new())).unwrap();
    }
    log.store(&stored(&before)).unwrap();
    let mut nor = log.release();
    nor.power = Some(power);
    let (mut log, _) = boot(nor);
    let _ = log.store(&stored(&during));
    let mut nor = log.release();
    nor.power = None;
    let (mut log, mem) = boot(nor);
    let got = read_back(&mem);
    prop_assert!(got == before || got == during, "lost both: {:?}", got.keys());
    log.store(&stored(&after)).unwrap();
    let (_, mem) = boot(log.release());
    prop_assert_eq!(read_back(&mem), after);
    Ok(())
}

proptest! {
    #[test]
    fn restore_returns_the_last_saved_files_byte_writes(ops in ops()) { replay::<1>(ops)?; }

    #[test]
    fn restore_returns_the_last_saved_files_word_writes(ops in ops()) { replay::<4>(ops)?; }

    #[test]
    fn restore_returns_the_last_saved_files_page_writes(ops in ops()) { replay::<256>(ops)?; }

    #[test]
    fn a_power_cut_mid_save_keeps_the_old_or_the_new_files_byte_writes(b in file_sets(), d in file_sets(), a in file_sets(), power in 0usize..5000) {
        cut::<1>(b, d, a, power)?;
    }

    #[test]
    fn a_power_cut_mid_save_keeps_the_old_or_the_new_files_page_writes(b in file_sets(), d in file_sets(), a in file_sets(), power in 0usize..5000) {
        cut::<256>(b, d, a, power)?;
    }
}

#[test]
fn a_cut_between_erase_and_write_keeps_the_previous_save() {
    let set = |k: &str, v: u8| Files::from([(k.to_string(), vec![v; 1500])]);
    for power in 0..1600 {
        cut::<1>(set("a", 1), set("b", 2), set("c", 3), power).unwrap();
    }
}
