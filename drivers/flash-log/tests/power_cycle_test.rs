use std::convert::Infallible;

use embedded_storage::nor_flash::{ErrorType, NorFlash, ReadNorFlash};
use posara_core::fs::{MemStorage, Storage};
use posara_flash_log::FlashLog;

const BASE: u32 = 8192;
const SECTOR: u32 = 4096;

struct Nor { bits: Vec<u8>, erases: usize, writes: usize }

impl Nor {
    fn new() -> Self { Self { bits: vec![0xFF; 3 * SECTOR as usize], erases: 0, writes: 0 } }
}
impl ErrorType for Nor { type Error = Infallible; }
impl ReadNorFlash for Nor {
    const READ_SIZE: usize = 1;
    fn read(&mut self, off: u32, buf: &mut [u8]) -> Result<(), Infallible> {
        buf.copy_from_slice(&self.bits[off as usize..off as usize + buf.len()]);
        Ok(())
    }
    fn capacity(&self) -> usize { self.bits.len() }
}
impl NorFlash for Nor {
    const WRITE_SIZE: usize = 1;
    const ERASE_SIZE: usize = SECTOR as usize;
    fn erase(&mut self, from: u32, to: u32) -> Result<(), Infallible> {
        assert!(from % SECTOR == 0 && to % SECTOR == 0);
        assert!(from >= BASE && to <= BASE + SECTOR, "erase outside the log");
        self.bits[from as usize..to as usize].fill(0xFF);
        self.erases += 1;
        Ok(())
    }
    fn write(&mut self, off: u32, data: &[u8]) -> Result<(), Infallible> {
        assert!(off >= BASE && off as usize + data.len() <= (BASE + SECTOR) as usize, "write outside the log");
        for (b, d) in self.bits[off as usize..].iter_mut().zip(data) { *b &= d; }
        self.writes += 1;
        Ok(())
    }
}

fn files(pairs: &[(&str, &[u8])]) -> MemStorage {
    let m = MemStorage::new();
    for (n, d) in pairs { m.add_file(n, d.to_vec()).unwrap(); }
    m
}

fn boot(nor: Nor) -> (FlashLog<Nor>, MemStorage) {
    let mut log = FlashLog::new(nor, BASE, SECTOR);
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
    let last = (BASE..BASE + SECTOR).rev().find(|&a| nor.bits[a as usize] != 0xFF).unwrap();
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
