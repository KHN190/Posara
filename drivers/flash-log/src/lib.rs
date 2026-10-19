#![no_std]

extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;
use embedded_storage::nor_flash::NorFlash;
use posara_core::fs::Storage;

const MAGIC: u8 = 0x5A;
const HEAD: usize = 4;

#[derive(Debug)]
pub enum Error<E> { Flash(E), TooBig }

// Record = MAGIC · len u16 LE · sum8 · files; files = count · (name_len · name · len u16 LE · data)*.
pub struct FlashLog<F> {
    flash: F,
    base: u32,
    size: u32,
    end: u32,
    last: Vec<u8>,
}

impl<F: NorFlash> FlashLog<F> {
    pub fn new(flash: F, base: u32, size: u32) -> Self {
        Self { flash, base, size, end: 0, last: vec![0] }
    }

    pub fn flash(&self) -> &F { &self.flash }
    pub fn release(self) -> F { self.flash }

    pub fn restore(&mut self, into: &dyn Storage) -> Result<(), Error<F::Error>> {
        let mut at = 0u32;
        loop {
            self.end = at;
            if at as usize + HEAD > self.size as usize { break; }
            let mut h = [0u8; HEAD];
            self.flash.read(self.base + at, &mut h).map_err(Error::Flash)?;
            if h == [0xFF; HEAD] { break; }
            let len = u16::from_le_bytes([h[1], h[2]]) as usize;
            if h[0] != MAGIC || at as usize + HEAD + len > self.size as usize {
                self.end = self.size;
                break;
            }
            let mut body = vec![0; len];
            self.flash.read(self.base + at + HEAD as u32, &mut body).map_err(Error::Flash)?;
            if sum(&body) != h[3] {
                self.end = self.size;
                break;
            }
            self.last = body;
            at += stride::<F>(len);
        }
        for (name, data) in decode(&self.last) {
            let _ = into.add_file(name, data.to_vec());
        }
        Ok(())
    }

    // Appends only when the files changed; erases the sector when the record no longer fits.
    pub fn store(&mut self, from: &dyn Storage) -> Result<bool, Error<F::Error>> {
        let names = from.file_names();
        let files: Vec<_> = names.iter().map(|n| (n.as_str(), from.read_file(n).unwrap_or_default())).collect();
        let body = encode(&files).ok_or(Error::TooBig)?;
        if body == self.last { return Ok(false); }
        let step = stride::<F>(body.len());
        if step > self.size { return Err(Error::TooBig); }
        if self.end + step > self.size {
            self.flash.erase(self.base, self.base + self.size).map_err(Error::Flash)?;
            self.end = 0;
        }
        let mut rec = Vec::with_capacity(step as usize);
        rec.push(MAGIC);
        rec.extend_from_slice(&(body.len() as u16).to_le_bytes());
        rec.push(sum(&body));
        rec.extend_from_slice(&body);
        rec.resize(step as usize, 0xFF);
        self.flash.write(self.base + self.end, &rec).map_err(Error::Flash)?;
        self.end += step;
        self.last = body;
        Ok(true)
    }
}

fn stride<F: NorFlash>(len: usize) -> u32 { (HEAD + len).next_multiple_of(F::WRITE_SIZE) as u32 }

fn sum(b: &[u8]) -> u8 { b.iter().fold(0u8, |a, x| a.wrapping_add(*x)) }

fn encode<N: AsRef<str>, D: AsRef<[u8]>>(files: &[(N, D)]) -> Option<Vec<u8>> {
    let mut out = vec![u8::try_from(files.len()).ok()?];
    for (n, d) in files {
        let (n, d) = (n.as_ref().as_bytes(), d.as_ref());
        out.push(u8::try_from(n.len()).ok()?);
        out.extend_from_slice(n);
        out.extend_from_slice(&u16::try_from(d.len()).ok()?.to_le_bytes());
        out.extend_from_slice(d);
    }
    u16::try_from(out.len()).ok()?;
    Some(out)
}

fn decode(b: &[u8]) -> Vec<(&str, &[u8])> {
    let mut out = Vec::new();
    let Some((&count, mut rest)) = b.split_first() else { return out };
    for _ in 0..count {
        let Some((&nl, r)) = rest.split_first() else { break };
        let Some((name, r)) = r.split_at_checked(nl as usize) else { break };
        let Some((len, r)) = r.split_at_checked(2) else { break };
        let Some((data, r)) = r.split_at_checked(u16::from_le_bytes([len[0], len[1]]) as usize) else { break };
        let Ok(name) = core::str::from_utf8(name) else { break };
        out.push((name, data));
        rest = r;
    }
    out
}
