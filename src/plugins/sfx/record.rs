use std::fs::File;
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::sfx::output::SampleRing;

// Streaming WAV writer. f32 samples come from the audio callback via SampleRing
// (lock-free SPSC). Each `drain_to_file` call pulls all available samples,
// converts to i16 LE, and appends to the file. On stop, RIFF/data chunk sizes
// in the header are back-filled.
pub struct Recorder {
    fd: BufWriter<File>,
    samples_written: u64,
    sample_rate: u32,
    channels: u16,
    ring: SampleRing,
    enabled: Arc<AtomicBool>,
}

impl Recorder {
    pub fn start(
        path: &Path,
        sample_rate: u32,
        channels: u16,
        ring: SampleRing,
        enabled: Arc<AtomicBool>,
    ) -> Result<Self, String> {
        let f = File::create(path).map_err(|e| e.to_string())?;
        let mut fd = BufWriter::new(f);
        write_wav_header(&mut fd, sample_rate, channels, 0)?;
        while ring.try_pop().is_some() {}
        enabled.store(true, Ordering::Release);
        Ok(Self { fd, samples_written: 0, sample_rate, channels, ring, enabled })
    }

    pub fn drain(&mut self) -> Result<(), String> {
        let mut buf = Vec::with_capacity(4096);
        while let Some(s) = self.ring.try_pop() {
            let i = (s.clamp(-1.0, 1.0) * 32767.0) as i16;
            buf.extend_from_slice(&i.to_le_bytes());
            self.samples_written += 1;
            if buf.len() >= 4096 {
                self.fd.write_all(&buf).map_err(|e| e.to_string())?;
                buf.clear();
            }
        }
        if !buf.is_empty() {
            self.fd.write_all(&buf).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    // Idempotent: safe to call twice. Drop also calls this as a safety net.
    pub fn stop(&mut self) -> Result<(), String> {
        if !self.enabled.swap(false, Ordering::AcqRel) {
            return Ok(());
        }
        self.drain()?;
        let data_size = (self.samples_written * 2) as u32;
        let riff_size = 36 + data_size;
        self.fd.flush().map_err(|e| e.to_string())?;
        let inner = self.fd.get_mut();
        inner.seek(SeekFrom::Start(4)).map_err(|e| e.to_string())?;
        inner.write_all(&riff_size.to_le_bytes()).map_err(|e| e.to_string())?;
        inner.seek(SeekFrom::Start(40)).map_err(|e| e.to_string())?;
        inner.write_all(&data_size.to_le_bytes()).map_err(|e| e.to_string())?;
        let _ = self.sample_rate;
        let _ = self.channels;
        Ok(())
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        if self.enabled.load(Ordering::Acquire) {
            eprintln!("recorder dropped without stop — backfilling WAV header");
            let _ = self.stop();
        }
    }
}

fn write_wav_header(
    fd: &mut BufWriter<File>,
    sample_rate: u32,
    channels: u16,
    data_size: u32,
) -> Result<(), String> {
    let bits: u16 = 16;
    let byte_rate: u32 = sample_rate * channels as u32 * (bits as u32 / 8);
    let block_align: u16 = channels * bits / 8;
    let riff_size: u32 = 36 + data_size;
    let mut w = |b: &[u8]| fd.write_all(b).map_err(|e| e.to_string());
    w(b"RIFF")?;
    w(&riff_size.to_le_bytes())?;
    w(b"WAVE")?;
    w(b"fmt ")?;
    w(&16u32.to_le_bytes())?;
    w(&1u16.to_le_bytes())?;
    w(&channels.to_le_bytes())?;
    w(&sample_rate.to_le_bytes())?;
    w(&byte_rate.to_le_bytes())?;
    w(&block_align.to_le_bytes())?;
    w(&bits.to_le_bytes())?;
    w(b"data")?;
    w(&data_size.to_le_bytes())?;
    Ok(())
}
