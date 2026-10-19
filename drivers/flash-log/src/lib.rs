#![no_std]

extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;
use embedded_storage::nor_flash::NorFlash;
use posara_core::fs::Storage;

const MAGIC: u8 = 0x5A;
const HEAD: usize = 12;

#[derive(Debug)]
pub enum Error<E> { Flash(E), TooBig }

// Two sectors, written alternately so a cut save never loses the previous one.
// Record = MAGIC · 0 · len u16 · seq u32 · fnv32(len, seq, files) · files;
// files = count · (name_len · name · len u16 · data)*. All LE.
pub struct FlashLog<F> {
    flash: F,
    base: u32,
    half: u32,
    sector: u32,
    end: u32,
    seq: u32,
    last: Vec<u8>,
}

impl<F: NorFlash> FlashLog<F> {
    pub fn new(flash: F, base: u32, size: u32) -> Self {
        Self { flash, base, half: size / 2, sector: 0, end: 0, seq: 0, last: vec![0] }
    }

    pub fn flash(&self) -> &F { &self.flash }
    pub fn release(self) -> F { self.flash }

    pub fn restore(&mut self, into: &dyn Storage) -> Result<(), Error<F::Error>> {
        let mut ends = [0u32; 2];
        let mut best: Option<(u32, u32, Vec<u8>)> = None;
        for s in 0..2 {
            let (end, latest) = self.scan(s)?;
            ends[s as usize] = end;
            if let Some((seq, body)) = latest {
                if best.as_ref().is_none_or(|b| seq > b.0) { best = Some((seq, s, body)); }
            }
        }
        if let Some((seq, s, body)) = best {
            (self.seq, self.sector, self.last) = (seq, s, body);
        }
        self.end = ends[self.sector as usize];
        for (name, data) in decode(&self.last) {
            let _ = into.add_file(name, data.to_vec());
        }
        Ok(())
    }

    fn scan(&mut self, s: u32) -> Result<(u32, Option<(u32, Vec<u8>)>), Error<F::Error>> {
        let start = self.base + s * self.half;
        let (mut at, mut latest) = (0u32, None);
        loop {
            if (at as usize + HEAD) > self.half as usize { return Ok((at, latest)); }
            let mut h = [0u8; HEAD];
            self.flash.read(start + at, &mut h).map_err(Error::Flash)?;
            if h == [0xFF; HEAD] { return Ok((at, latest)); }
            let len = u16::from_le_bytes([h[2], h[3]]) as usize;
            if h[0] != MAGIC || at as usize + HEAD + len > self.half as usize { return Ok((self.half, latest)); }
            let seq = u32::from_le_bytes([h[4], h[5], h[6], h[7]]);
            let mut body = vec![0; len];
            self.flash.read(start + at + HEAD as u32, &mut body).map_err(Error::Flash)?;
            if fnv(len as u16, seq, &body) != u32::from_le_bytes([h[8], h[9], h[10], h[11]]) { return Ok((self.half, latest)); }
            latest = Some((seq, body));
            at += stride::<F>(len);
        }
    }

    // Appends only when the files changed; a full sector moves on to the other one.
    pub fn store(&mut self, from: &dyn Storage) -> Result<bool, Error<F::Error>> {
        let names = from.file_names();
        let files: Vec<_> = names.iter().map(|n| (n.as_str(), from.read_file(n).unwrap_or_default())).collect();
        let body = encode(&files).ok_or(Error::TooBig)?;
        if body == self.last { return Ok(false); }
        let step = stride::<F>(body.len());
        if step > self.half { return Err(Error::TooBig); }
        if self.end + step > self.half {
            let other = 1 - self.sector;
            let start = self.base + other * self.half;
            self.flash.erase(start, start + self.half).map_err(Error::Flash)?;
            (self.sector, self.end) = (other, 0);
        }
        self.seq = self.seq.wrapping_add(1);
        let len = body.len() as u16;
        let mut rec = Vec::with_capacity(step as usize);
        rec.extend_from_slice(&[MAGIC, 0]);
        rec.extend_from_slice(&len.to_le_bytes());
        rec.extend_from_slice(&self.seq.to_le_bytes());
        rec.extend_from_slice(&fnv(len, self.seq, &body).to_le_bytes());
        rec.extend_from_slice(&body);
        rec.resize(step as usize, 0xFF);
        self.flash.write(self.base + self.sector * self.half + self.end, &rec).map_err(Error::Flash)?;
        self.end += step;
        self.last = body;
        Ok(true)
    }
}

fn stride<F: NorFlash>(len: usize) -> u32 { (HEAD + len).next_multiple_of(F::WRITE_SIZE) as u32 }

fn fnv(len: u16, seq: u32, body: &[u8]) -> u32 {
    len.to_le_bytes().iter().chain(&seq.to_le_bytes()).chain(body)
        .fold(0x811C_9DC5u32, |h, &b| (h ^ b as u32).wrapping_mul(0x0100_0193))
}

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
