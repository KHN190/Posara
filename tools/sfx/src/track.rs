// `posara-sfx track` — .mid -> .trk packed events.
//   cart: let t = fs_read(fd, N); snd_track(&t, ms_per_tick)
// each event = one i64: tick(16) | ch(3) | note(8) | vol(7) | dur(16).

use std::path::PathBuf;
use std::process::ExitCode;

use midly::{MetaMessage, MidiMessage, Smf, Timing, TrackEventKind};

fn usage() -> ExitCode {
    eprintln!("usage: posara-sfx track <in.mid> <out.trk> [opts]");
    eprintln!("  --voices N      max channels 1..4 (default 4)");
    eprintln!("  --project MODE  voice|pitch (default voice)");
    eprintln!("                    voice = temporal allocator, steals when polyphony > voices");
    eprintln!("                    pitch = route by pitch percentile to ch 0..N-1, no stealing");
    eprintln!("  --transpose N   semitone shift (default 0)");
    eprintln!("  --res N         our ticks-per-quarter (default 4 = 16th-note grid)");
    eprintln!("  --tempo BPM     override MIDI initial tempo");
    ExitCode::from(2)
}

#[derive(Clone, Copy, PartialEq)]
enum Project { Voice, Pitch }

pub fn run(args: Vec<String>) -> ExitCode {
    let mut pos: Vec<String> = Vec::new();
    let mut voices = 4usize;
    let mut transpose: i32 = 0;
    let mut res: u32 = 4;
    let mut tempo_override: Option<u32> = None;
    let mut project = Project::Voice;
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        let mut next = || it.next().ok_or_else(usage);
        match a.as_str() {
            "-h" | "--help" => return usage(),
            "--voices"    => match next() { Ok(v) => match v.parse::<usize>() { Ok(n) => voices = n.clamp(1, 4), _ => return usage() }, Err(c) => return c },
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

    let alloc: Vec<usize> = match project {
        Project::Voice => {
            #[derive(Clone, Copy)]
            struct Slot { end: u64, midi_ch: Option<u8> }
            let mut slots = vec![Slot { end: 0, midi_ch: None }; voices];
            let mut alloc: Vec<usize> = Vec::with_capacity(notes.len());
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
                    slots.iter().enumerate().min_by_key(|(_, s)| s.end).map(|(i, _)| i).unwrap()
                });
                slots[p] = Slot { end: n.start + n.dur, midi_ch: Some(n.ch) };
                alloc.push(p);
            }
            alloc
        }
        Project::Pitch => {
            let mut sorted: Vec<u8> = notes.iter().map(|n| n.key).collect();
            sorted.sort();
            let cuts: Vec<u8> = (1..voices).map(|i| sorted[sorted.len() * i / voices]).collect();
            let band = |k: u8| -> usize {
                cuts.iter().position(|&c| k < c).unwrap_or(voices - 1)
            };
            notes.iter().map(|n| band(n.key)).collect()
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
    let mut raw = Vec::with_capacity(events.len() * 8);
    for &e in &events { raw.extend_from_slice(&e.to_le_bytes()); }
    if let Err(e) = std::fs::write(&out, &raw) {
        eprintln!("write {}: {e}", out.display());
        return ExitCode::from(1);
    }
    eprintln!(
        "wrote {} ({} events, {} voices, {} bytes; cart: let t = fs_read(fd, {}); snd_track(&t, {}))",
        out.display(), events.len(), used, events.len() * 8, events.len() * 8, our_ms_per_tick
    );
    if preempted > 0 {
        let pct = preempted as f64 * 100.0 / events.len().max(1) as f64;
        let cause = match project {
            Project::Voice => "voice stealing under dense polyphony — try --project pitch",
            Project::Pitch => "same-pitch-band collisions — raise --voices or accept later-wins",
        };
        eprintln!("warn: {preempted}/{} notes preempted ({:.0}%) — {}", events.len(), pct, cause);
    }
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

#[cfg(test)]
mod tests {
    use super::pack;

    #[test]
    fn fields_land_in_their_bits() {
        let w = pack(0x1234, 3, 60, 100, 0x0abc);
        assert_eq!(w & 0xFFFF, 0x1234);          // tick
        assert_eq!((w >> 16) & 0x7, 3);          // ch
        assert_eq!((w >> 19) & 0xFF, 60);        // note
        assert_eq!((w >> 27) & 0x7F, 100);       // vol
        assert_eq!((w >> 34) & 0xFFFF, 0x0abc);  // dur
    }
}
