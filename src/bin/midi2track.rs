// midi2track — .mid → sfx_seq / sfx_track
//
// output:
//   *.abe  → use sfx_inst + sfx_seq in cart
//   *      → raw .trk, a cart loads with fs_read + sfx_track

use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use midly::{MetaMessage, MidiMessage, Smf, Timing, TrackEventKind};

fn usage() -> ExitCode {
    eprintln!("usage: midi2track <in.mid> <out.abe|.trk> [opts]");
    eprintln!("  --voices N      max sfx channels 1..4 (default 4)");
    eprintln!("  --project MODE  voice|pitch (default voice)");
    eprintln!("                    voice = temporal allocator, steals when polyphony > voices");
    eprintln!("                    pitch = route by pitch percentile to ch 0..N-1 (low..high), no stealing");
    eprintln!("  --wave K        cart waveform 0..4 for .abe output (default 0=square)");
    eprintln!("  --transpose N   semitone shift (default 0)");
    eprintln!("  --res N         our ticks-per-quarter (default 4 = 16th-note grid)");
    eprintln!("  --tempo BPM     override MIDI initial tempo");
    ExitCode::from(2)
}

#[derive(Clone, Copy, PartialEq)]
enum Project { Voice, Pitch }

fn main() -> ExitCode {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let mut pos: Vec<String> = Vec::new();
    let mut voices = 4usize;
    let mut wave = 0i64;
    let mut transpose: i32 = 0;
    let mut res: u32 = 4;
    let mut tempo_override: Option<u32> = None;
    let mut project = Project::Voice;
    let mut it = raw.into_iter();
    while let Some(a) = it.next() {
        let mut next = || it.next().ok_or_else(usage);
        match a.as_str() {
            "-h" | "--help" => return usage(),
            "--voices"    => match next() { Ok(v) => match v.parse::<usize>() { Ok(n) => voices = n.clamp(1, 4), _ => return usage() }, Err(c) => return c },
            "--wave"      => match next() { Ok(v) => match v.parse::<i64>() { Ok(n) => wave = n.clamp(0, 4), _ => return usage() }, Err(c) => return c },
            "--transpose" => match next() { Ok(v) => match v.parse::<i32>() { Ok(n) => transpose = n, _ => return usage() }, Err(c) => return c },
            "--res"       => match next() { Ok(v) => match v.parse::<u32>() { Ok(n) if n > 0 => res = n, _ => return usage() }, Err(c) => return c },
            "--tempo"     => match next() { Ok(v) => match v.parse::<f32>() { Ok(b) if b > 0.0 => tempo_override = Some((60_000_000.0 / b) as u32), _ => return usage() }, Err(c) => return c },
            "--project"   => match next() {
                Ok(v) => match v.as_str() { "voice" => project = Project::Voice, "pitch" => project = Project::Pitch, _ => return usage() },
                Err(c) => return c,
            },
            _ => pos.push(a),
        }
    }
    if pos.len() != 2 { return usage(); }
    let inp = PathBuf::from(&pos[0]);
    let out = PathBuf::from(&pos[1]);

    let bytes = match std::fs::read(&inp) {
        Ok(b) => b,
        Err(e) => { eprintln!("read {}: {e}", inp.display()); return ExitCode::from(1); }
    };
    let smf = match Smf::parse(&bytes) {
        Ok(s) => s,
        Err(e) => { eprintln!("parse MIDI: {e}"); return ExitCode::from(1); }
    };
    let ppq = match smf.header.timing {
        Timing::Metrical(t) => t.as_int() as u32,
        Timing::Timecode(_, _) => { eprintln!("SMPTE timing not supported"); return ExitCode::from(1); }
    };
    if ppq == 0 { eprintln!("zero PPQ"); return ExitCode::from(1); }

    let mut tempo_us: u32 = 500_000;
    'tempo: for track in &smf.tracks {
        for ev in track {
            if let TrackEventKind::Meta(MetaMessage::Tempo(t)) = ev.kind {
                tempo_us = t.as_int();
                break 'tempo;
            }
        }
    }
    if let Some(t) = tempo_override { tempo_us = t; }

    struct Note { ch: u8, key: u8, start: u64, dur: u64, vel: u8 }
    let mut notes: Vec<Note> = Vec::new();
    let mut cc7: Vec<Vec<(u64, u8)>> = vec![Vec::new(); 16];
    let mut cc11: Vec<Vec<(u64, u8)>> = vec![Vec::new(); 16];
    for track in &smf.tracks {
        let mut t: u64 = 0;
        let mut open: [[Option<(u64, u8)>; 128]; 16] = [[None; 128]; 16];
        for ev in track {
            t += ev.delta.as_int() as u64;
            if let TrackEventKind::Midi { channel, message } = ev.kind {
                let ch = channel.as_int() as usize;
                match message {
                    MidiMessage::NoteOn { key, vel } => {
                        let k = key.as_int();
                        let v = vel.as_int();
                        if v == 0 {
                            if let Some((s, v0)) = open[ch][k as usize].take() {
                                notes.push(Note { ch: ch as u8, key: k, start: s, dur: t - s, vel: v0 });
                            }
                        } else {
                            if let Some((s, v0)) = open[ch][k as usize].take() {
                                notes.push(Note { ch: ch as u8, key: k, start: s, dur: t - s, vel: v0 });
                            }
                            open[ch][k as usize] = Some((t, v));
                        }
                    }
                    MidiMessage::NoteOff { key, .. } => {
                        let k = key.as_int() as usize;
                        if let Some((s, v0)) = open[ch][k].take() {
                            notes.push(Note { ch: ch as u8, key: k as u8, start: s, dur: t - s, vel: v0 });
                        }
                    }
                    MidiMessage::Controller { controller, value } => {
                        match controller.as_int() {
                            7  => cc7[ch].push((t, value.as_int())),
                            11 => cc11[ch].push((t, value.as_int())),
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    if notes.is_empty() { eprintln!("no notes found"); return ExitCode::from(1); }

    notes.sort_by_key(|n| n.start);
    for v in cc7.iter_mut().chain(cc11.iter_mut()) { v.sort_by_key(|p| p.0); }

    let (alloc, stolen): (Vec<usize>, u32) = match project {
        Project::Voice => {
            #[derive(Clone, Copy)]
            struct Slot { end: u64, midi_ch: Option<u8> }
            let mut slots = vec![Slot { end: 0, midi_ch: None }; voices];
            let mut alloc: Vec<usize> = Vec::with_capacity(notes.len());
            let mut stolen = 0u32;
            for n in &notes {
                let mut pick: Option<usize> = None;
                for (i, s) in slots.iter().enumerate() {
                    if s.end <= n.start && s.midi_ch == Some(n.ch) { pick = Some(i); break; }
                }
                if pick.is_none() {
                    for (i, s) in slots.iter().enumerate() {
                        if s.end <= n.start { pick = Some(i); break; }
                    }
                }
                let p = pick.unwrap_or_else(|| {
                    stolen += 1;
                    slots.iter().enumerate().min_by_key(|(_, s)| s.end).map(|(i, _)| i).unwrap()
                });
                slots[p] = Slot { end: n.start + n.dur, midi_ch: Some(n.ch) };
                alloc.push(p);
            }
            (alloc, stolen)
        }
        Project::Pitch => {
            let mut sorted: Vec<u8> = notes.iter().map(|n| n.key).collect();
            sorted.sort();
            let cuts: Vec<u8> = (1..voices).map(|i| sorted[sorted.len() * i / voices]).collect();
            let band = |k: u8| -> usize {
                cuts.iter().position(|&c| k < c).unwrap_or(voices - 1)
            };
            (notes.iter().map(|n| band(n.key)).collect(), 0)
        }
    };

    let our_ms_per_tick = (tempo_us as f64 / 1000.0 / res as f64).round().max(1.0) as i64;
    let to_our = |mt: u64| ((mt * res as u64 + ppq as u64 / 2) / ppq as u64) as i64;

    let mut events: Vec<i64> = Vec::with_capacity(notes.len());
    let mut overflow = 0u32;
    let mut preempted = 0u32;
    let mut last_idx: Vec<Option<usize>> = vec![None; voices];
    for (n, &och) in notes.iter().zip(alloc.iter()) {
        let tick = to_our(n.start);
        let dur = to_our(n.dur).max(1);
        if tick >= 1 << 16 || dur >= 1 << 16 { overflow += 1; continue; }
        if let Some(pi) = last_idx[och] {
            let w = events[pi];
            let prev_tick = w & 0xFFFF;
            let prev_dur = (w >> 34) & 0xFFFF;
            if prev_tick + prev_dur > tick {
                preempted += 1;
                let new_dur = (tick - prev_tick).max(1);
                events[pi] = (w & !(0xFFFFi64 << 34)) | ((new_dur & 0xFFFF) << 34);
            }
        }
        let note = (n.key as i32 + transpose).clamp(0, 127) as i64;
        let v7 = cc_at(&cc7[n.ch as usize], n.start) as i64;
        let v11 = cc_at(&cc11[n.ch as usize], n.start) as i64;
        let scaled = n.vel as i64 * v7 * v11 / (127 * 127);
        let vol = (scaled * 100 / 127).clamp(0, 100);
        events.push(pack(tick, och as i64, note, vol, dur));
        last_idx[och] = Some(events.len() - 1);
    }
    if events.is_empty() { eprintln!("all notes overflowed 16-bit ticks; try larger --res"); return ExitCode::from(1); }

    let used = alloc.iter().copied().max().map(|m| m + 1).unwrap_or(1);
    let raw_out = out.extension().and_then(|s| s.to_str()) != Some("abe");
    let res_io = if raw_out {
        write_raw(&out, &events)
    } else {
        write_cart(&out, &events, our_ms_per_tick, wave, used)
    };
    if let Err(e) = res_io {
        eprintln!("write {}: {e}", out.display());
        return ExitCode::from(1);
    }
    if raw_out {
        eprintln!(
            "wrote {} ({} events, {} bytes; cart: let t = fs_read(fd, {}); sfx_track(t, {}))",
            out.display(), events.len(), events.len() * 8, events.len() * 8, our_ms_per_tick
        );
    } else {
        eprintln!(
            "wrote {} ({} events, {} voices, {} ms/tick; tempo {} BPM, PPQ {}, res {}) → posara run {}",
            out.display(), events.len(), used, our_ms_per_tick,
            60_000_000 / tempo_us, ppq, res, out.display()
        );
    }
    if preempted > 0 {
        let pct = preempted as f64 * 100.0 / events.len().max(1) as f64;
        let cause = match project {
            Project::Voice => "voice stealing under dense polyphony — try --project pitch",
            Project::Pitch => "same-pitch-band collisions — raise --voices or accept later-wins",
        };
        eprintln!("warn: {preempted}/{} notes preempted ({:.0}%) — {}", events.len(), pct, cause);
    }
    let _ = stolen;
    if overflow > 0 { eprintln!("warn: {overflow} note(s) past 16-bit tick/dur — try larger --res"); }
    ExitCode::SUCCESS
}

fn cc_at(timeline: &[(u64, u8)], tick: u64) -> u8 {
    match timeline.binary_search_by_key(&tick, |&(t, _)| t) {
        Ok(i) => timeline[i].1,
        Err(0) => 127,
        Err(i) => timeline[i - 1].1,
    }
}

fn pack(tick: i64, ch: i64, note: i64, vol: i64, dur: i64) -> i64 {
    (tick & 0xFFFF) | ((ch & 0x7) << 16) | ((note & 0xFF) << 19) | ((vol & 0x7F) << 27) | ((dur & 0xFFFF) << 34)
}

fn write_raw(out: &PathBuf, events: &[i64]) -> std::io::Result<()> {
    let mut bytes = Vec::with_capacity(events.len() * 8);
    for &e in events { bytes.extend_from_slice(&e.to_le_bytes()); }
    std::fs::write(out, &bytes)
}

fn write_cart(out: &PathBuf, events: &[i64], ms_per_tick: i64, wave: i64, voices: usize) -> std::io::Result<()> {
    let max_end_ticks: i64 = events.iter().map(|&w| {
        let tick = w & 0xFFFF;
        let dur = (w >> 34) & 0xFFFF;
        tick + dur
    }).max().unwrap_or(0);
    let total_ms = max_end_ticks * ms_per_tick.max(1) + 1000;

    let mut f = std::fs::File::create(out)?;
    writeln!(f, "// generated by midi2track")?;
    writeln!(f, "// {} events, {} voices, {} ms/tick, ~{} ms total", events.len(), voices, ms_per_tick, total_ms)?;
    writeln!(f)?;
    writeln!(f, "fn main() -> Unit {{ () }}")?;
    writeln!(f)?;
    writeln!(f, "pub fn start() -> <Graphics, IO> Unit {{")?;
    writeln!(f, "  screen_off();")?;
    writeln!(f, "  screen(160, 120);")?;
    writeln!(f, "  cls(0x0000);")?;
    for c in 0..voices {
        writeln!(f, "  sfx_inst({}, {}, 2, 0, 100, 80);", c, wave)?;
    }
    write!(f, "  sfx_seq([")?;
    for (i, &e) in events.iter().enumerate() {
        if i > 0 { write!(f, ",")?; }
        if i % 8 == 0 { write!(f, "\n    ")?; }
        write!(f, "{}", e)?;
    }
    writeln!(f, "\n  ], {})", ms_per_tick.max(1))?;
    writeln!(f, "}}")?;
    writeln!(f)?;
    writeln!(f, "pub fn update() -> <IO> Unit {{")?;
    writeln!(f, "  if now() >= {} {{ halt(0) }}", total_ms)?;
    writeln!(f, "}}")?;
    Ok(())
}
