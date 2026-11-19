use alloc::{boxed::Box, vec};
use crate::math;

use super::env::Adsr;
use super::fx::Fx;
use super::lfo::Lfo;
use super::seq::note_to_freq;

const MAX_PATCHES: usize = 16;
pub const MAX_VOICES: usize = 32;
pub const MAX_UNISON: usize = 7;

const FK_HI: u8 = 1;
const FK_BAND: u8 = 2;
const FK_LPG: u8 = 3;

const TGT_AMP: u8 = 0;
pub const TGT_AMP_EXP: u8 = 3;
const TGT_CUTOFF: u8 = 1;
const TGT_PITCH: u8 = 2;

#[derive(Clone, Copy)]
struct OscCfg {
    wave: u8,
    #[cfg(any(feature = "rp32", all(target_os = "none", target_pointer_width = "32")))]
    timbre: u8,
    mult: f32,
    level: f32,
}

impl Default for OscCfg {
    fn default() -> Self {
        Self {
            wave: 3,
            #[cfg(any(feature = "rp32", all(target_os = "none", target_pointer_width = "32")))]
            timbre: 0,
            mult: 1.0,
            level: 0.0,
        }
    }
}

#[derive(Clone, Copy, Default)]
struct EnvCfg {
    target: u8,
    depth: f32,
    atk: f32,
    dec: f32,
    sus: f32,
    rel: f32,
}

#[derive(Clone, Copy)]
struct Patch {
    osc: [OscCfg; 2],
    fkind: u8,
    cutoff: f32,
    reso: f32,
    env: [EnvCfg; 2],
    lfo: Lfo,
    unison: u8,
    detune: f32,
    fx_kind: u8,
    fx_amt: f32,
    fx_param: f32,
    pan: f32,   // -1 left .. 0 centre .. 1 right
    pan_l: f32,
    pan_r: f32,
}

impl Default for Patch {
    fn default() -> Self {
        let mut osc0 = OscCfg::default();
        osc0.level = 1.0;
        Self {
            osc: [osc0, OscCfg::default()],
            fkind: 0,
            cutoff: 18000.0,
            reso: 0.0,
            env: [
                EnvCfg { target: TGT_AMP, depth: 100.0, atk: 0.0, dec: 0.0, sus: 1.0, rel: 0.05 },
                // slot1 defaults to an inert cutoff mod (depth 0) — never silences a
                // patch that only configures slot0. Default-AMP here would zero amp.
                EnvCfg { target: TGT_CUTOFF, depth: 0.0, atk: 0.0, dec: 0.0, sus: 0.0, rel: 0.0 },
            ],
            lfo: Lfo::default(),
            unison: 1,
            detune: 0.0,
            fx_kind: 0,
            fx_amt: 0.0,
            fx_param: 0.0,
            pan: 0.0,
            pan_l: core::f32::consts::FRAC_1_SQRT_2,
            pan_r: core::f32::consts::FRAC_1_SQRT_2,
        }
    }
}

#[derive(Clone, Copy)]
struct Voice {
    modulation: [f32; 3],
    modulation_target: [f32; 3],
    #[cfg(any(feature = "rp32", all(target_os = "none", target_pointer_width = "32")))]
    modulation_delta: [f32; 3],
    #[cfg(any(feature = "rp32", all(target_os = "none", target_pointer_width = "32")))]
    modulation_left: u8,
    modulation_active: bool,
    cfg: Patch,
    pid: usize,
    note: u8,
    vel: f32,
    active: bool,
    gate: bool,
    gate_samples: u32,
    rank: u32,
    base_freq: f32,
    env: [Adsr; 2],
    env_used: [bool; 2],
    lfo: Lfo,
    fx: Fx,
    phase: [[f32; 2]; MAX_UNISON],
    uratio: [f32; MAX_UNISON],
    phase_step: [[f32; 2]; MAX_UNISON],
    step_freq: f32,
    step_sr: f32,
    ucount: usize,
    #[cfg(any(feature = "rp32", all(target_os = "none", target_pointer_width = "32")))]
    baked_fm: bool,
    /// Amplitude at the end of the last rendered block: the next block ramps from it (msfa gain1).
    kernel_gain: f32,
    lfsr: u32,
    flp: f32,
    fbp: f32,
    fixed_filter: bool,
    cached_fc: f32,
    cached_reso: f32,
    cached_filter_sr: f32,
    cached_f: f32,
    cached_damp: f32,
    pan_l: f32,
    pan_r: f32,
}

impl Default for Voice {
    fn default() -> Self {
        Self {
            modulation: [1.0, 1.0, 0.0],
            modulation_target: [1.0, 1.0, 0.0],
            #[cfg(any(feature = "rp32", all(target_os = "none", target_pointer_width = "32")))]
            modulation_delta: [0.0; 3],
            #[cfg(any(feature = "rp32", all(target_os = "none", target_pointer_width = "32")))]
            modulation_left: 0,
            modulation_active: false,
            cfg: Patch::default(),
            pid: 0,
            note: 0,
            vel: 0.0,
            active: false,
            gate: false,
            gate_samples: 0,
            rank: 0,
            base_freq: 0.0,
            env: [Adsr::default(); 2],
            env_used: [false; 2],
            lfo: Lfo::default(),
            fx: Fx::default(),
            phase: [[0.0; 2]; MAX_UNISON],
            uratio: [1.0; MAX_UNISON],
            phase_step: [[0.0; 2]; MAX_UNISON],
            step_freq: f32::NAN,
            step_sr: f32::NAN,
            ucount: 1,
            #[cfg(any(feature = "rp32", all(target_os = "none", target_pointer_width = "32")))]
            baked_fm: false,
            kernel_gain: 0.0,
            lfsr: 0x1234abcd,
            flp: 0.0,
            fbp: 0.0,
            fixed_filter: false,
            cached_fc: 0.0,
            cached_reso: 0.0,
            cached_filter_sr: 0.0,
            cached_f: 0.0,
            cached_damp: 0.0,
            pan_l: 0.0,
            pan_r: 0.0,
        }
    }
}

pub const WAVE_FM: u8 = 5;
/// Preview frequency FM with a fixed 2:1 modulator, separate from generic PM.
pub const WAVE_PREVIEW_FM: u8 = 6;
#[cfg(any(test, feature = "rp32", all(target_os = "none", target_pointer_width = "32")))]
#[path = "synth_tables.rs"]
mod synth_tables;

fn wave_sample(wave: u8, phase: f32, lfsr: &mut u32) -> f32 {
    if wave == 4 {
        let mut x = if *lfsr == 0 { 0x1234abcd } else { *lfsr };
        x ^= x << 13; x ^= x >> 17; x ^= x << 5;
        *lfsr = x;
        return ((x & 0xFF) as f32 / 128.0) - 1.0;
    }
    match wave {
        1 => math::oscillator_sin(phase),
        2 => if phase < 0.5 { 4.0 * phase - 1.0 } else { 3.0 - 4.0 * phase },
        3 => 2.0 * phase - 1.0,
        _ => if phase < 0.5 { 1.0 } else { -1.0 },
    }
}

impl Voice {
    fn start(&mut self, p: &Patch, pid: usize, note: u8, vel: f32, gate_samples: u32, seed: u32) {
        self.cfg = *p;
        self.kernel_gain = 0.0;
        self.step_freq = f32::NAN;
        self.step_sr = f32::NAN;
        self.pid = pid;
        self.note = note;
        self.vel = vel.clamp(0.0, 1.0);
        self.base_freq = note_to_freq(note);
        self.active = true;
        self.gate = true;
        self.gate_samples = gate_samples;
        self.rank = 0;
        for i in 0..2 {
            let e = p.env[i];
            self.env_used[i] = matches!(e.target, TGT_AMP | TGT_AMP_EXP) || e.depth != 0.0 || e.atk != 0.0
                || e.dec != 0.0 || e.sus != 0.0 || e.rel != 0.0;
            if e.target == TGT_AMP_EXP {
                self.env[i].set_exponential(e.atk, e.dec, e.sus, e.rel);
            } else {
                self.env[i].set(e.atk, e.dec, e.sus, e.rel);
            }
            self.env[i].gate_on();
        }
        self.lfo = p.lfo;
        // A voice owns a patch snapshot; only cutoff modulation can change its filter.
        self.fixed_filter = p.fkind != FK_LPG
            && !p.env.iter().any(|e| e.target == TGT_CUTOFF && e.depth != 0.0)
            && !(p.lfo.enabled() && p.lfo.target == TGT_CUTOFF);
        self.fx = Fx::default();
        self.fx.set(p.fx_kind, p.fx_amt, p.fx_param);
        let c = (p.unison.max(1) as usize).min(MAX_UNISON);
        self.ucount = c;
        #[cfg(any(feature = "rp32", all(target_os = "none", target_pointer_width = "32")))]
        {
            let [car, modu] = p.osc;
            let baked_level = (10 + 8 * modu.timbre) as f32 / 100.0;
            // A phase table requires the sine carrier and phase-locked 2:1 pair.
            self.baked_fm = c == 1 && car.wave == 1 && modu.mult == 2.0 * car.mult
                && (modu.level - baked_level).abs() <= f32::EPSILON;
        }
        if c == 1 {
            self.uratio[0] = 1.0;
            self.phase[0] = [0.0, 0.0];
        } else {
            for u in 0..c {
                let spread = (u as f32 / (c as f32 - 1.0)) * 2.0 - 1.0;
                self.uratio[u] = math::powf(2.0f32, spread * p.detune / 1200.0);
                self.phase[u] = [(u as f32 * 0.137) % 1.0, (u as f32 * 0.331) % 1.0];
            }
        }
        self.lfsr = 0x1234abcd ^ note as u32 ^ seed.wrapping_mul(2654435761);
        self.flp = 0.0;
        self.fbp = 0.0;
        self.cached_fc = 0.0;
        self.cached_reso = 0.0;
        self.cached_filter_sr = 0.0;
        self.cached_f = 0.0;
        self.cached_damp = 0.0;
        self.pan_l = p.pan_l;
        self.pan_r = p.pan_r;
    }

    fn note_off(&mut self) {
        self.gate = false;
        for e in self.env.iter_mut() { e.gate_off(); }
    }

    /// SVF coefficients for this cutoff, cached as in `filter`.
    fn filter_coeffs(&mut self, cutoff: f32, reso: f32, sr: f32) -> (f32, f32) {
        let fc = cutoff.clamp(20.0, sr * 0.16);
        if fc != self.cached_fc || reso != self.cached_reso || sr != self.cached_filter_sr {
            self.cached_f = 2.0 * math::oscillator_sin(0.5 * fc / sr);
            self.cached_damp = (1.0 - (reso / 100.0) * 0.9).clamp(0.1, 1.0);
            self.cached_fc = fc;
            self.cached_reso = reso;
            self.cached_filter_sr = sr;
        }
        (self.cached_f, self.cached_damp)
    }

    fn filter(&mut self, x: f32, cutoff: f32, reso: f32, sr: f32) -> f32 {
        // Chamberlin SVF: stable only for fc well below sr/6 — clamp to keep f < 1,
        // else it blows up to inf/NaN (silent output that also dodges the meter).
        if !self.fixed_filter || sr != self.cached_filter_sr {
            let fc = cutoff.clamp(20.0, sr * 0.16);
            if fc != self.cached_fc || reso != self.cached_reso || sr != self.cached_filter_sr {
                self.cached_f = 2.0 * math::oscillator_sin(0.5 * fc / sr);
                self.cached_damp = (1.0 - (reso / 100.0) * 0.9).clamp(0.1, 1.0);
                self.cached_fc = fc;
                self.cached_reso = reso;
                self.cached_filter_sr = sr;
            }
        }
        let f = self.cached_f;
        let damp = self.cached_damp;
        self.flp += f * self.fbp;
        let hp = x - self.flp - damp * self.fbp;
        self.fbp += f * hp;
        if !self.flp.is_finite() || !self.fbp.is_finite() { self.flp = 0.0; self.fbp = 0.0; }
        match self.cfg.fkind {
            FK_HI => hp,
            FK_BAND => self.fbp,
            _ => self.flp,
        }
    }

    /// One call per voice per block: the per-frame body inlines here (msfa FmCore::compute style).
    #[cfg_attr(all(target_arch = "arm", target_os = "none"), unsafe(link_section = ".data.ram_func"))]
    fn render(
        &mut self,
        sr: f32,
        dt: f32,
        #[cfg(any(feature = "rp32", all(target_os = "none", target_pointer_width = "32")))]
        fm_table: &[i16; 1025],
        chunk: &mut [(f32, f32)],
        psum: &mut [[f32; 4]],
        pid: usize,
    ) -> u32 {
        #[cfg(any(feature = "rp32", all(target_os = "none", target_pointer_width = "32")))]
        if self.single_fm_block(chunk.len()) {
            KERNEL_PROBE.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
            return if self.modulation[2] > 0.0 { self.render_single_fm::<true>(sr, dt, fm_table, chunk, psum, pid) } else { self.render_single_fm::<false>(sr, dt, fm_table, chunk, psum, pid) };
        }
        let mut rendered = 0;
        for (n, frame) in chunk.iter_mut().enumerate() {
            // Inactive voices return 0.0 exactly like the per-frame loop skipping them.
            if !self.active { break; }
            rendered += 1;
            let s = self.tick(
                sr,
                dt,
                #[cfg(any(feature = "rp32", all(target_os = "none", target_pointer_width = "32")))]
                fm_table,
            );
            frame.0 += s * self.pan_l;
            frame.1 += s * self.pan_r;
            if pid < 4 { psum[n][pid] += s; }
        }
        rendered
    }

    /// The builtin patch shape: one baked-FM unison voice, one amplitude envelope, fixed filter,
    /// no LFO, FX or pending gate edge in this block. Checked once per block (msfa specialised kernel).
    #[cfg(any(feature = "rp32", all(target_os = "none", target_pointer_width = "32")))]
    fn single_fm_block(&self, len: usize) -> bool {
        let modu = self.cfg.osc[1];
        self.active && !self.modulation_active && self.ucount == 1 && self.baked_fm
            && matches!(modu.wave, WAVE_FM | WAVE_PREVIEW_FM) && modu.level > 0.0
            && self.env_used == [true, false] && matches!(self.cfg.env[0].target, TGT_AMP | TGT_AMP_EXP)
            && !self.lfo.enabled() && self.cfg.fkind != FK_LPG
            && !(self.cfg.fx_kind != 0 && !(self.cfg.fx_amt <= 0.0))
            && (self.gate_samples == 0 || self.gate_samples as usize > len)
    }

    /// msfa FmOpKernel style: envelope once per block with a linear gain ramp, Q32 phase and
    /// integer table interpolation; the frame loop has no branches or float-to-int conversions.
    #[cfg(any(feature = "rp32", all(target_os = "none", target_pointer_width = "32")))]
    #[inline(always)]
    fn render_single_fm<const DRIVE: bool>(&mut self, sr: f32, dt: f32, fm_table: &[i16; 1025], chunk: &mut [(f32, f32)], psum: &mut [[f32; 4]], pid: usize) -> u32 {
        let len = chunk.len();
        let freq = self.base_freq;
        if freq.to_bits() != self.step_freq.to_bits() || sr.to_bits() != self.step_sr.to_bits() {
            let (car, modu) = (self.cfg.osc[0], self.cfg.osc[1]);
            let uf = freq * self.uratio[0];
            self.phase_step[0] = [uf * car.mult / sr, uf * modu.mult / sr];
            self.step_freq = freq;
            self.step_sr = sr;
        }
        const Q32: f32 = 4_294_967_296.0;
        let step = (self.phase_step[0][0] * self.modulation[1] * Q32) as u32;
        let mut phase = (self.phase[0][0] * Q32) as u32;
        let gain1 = self.kernel_gain;
        let gain2 = self.env[0].tick(len as f32 * dt);
        self.kernel_gain = gain2;
        let dgain = (gain2 - gain1) * (1.0 / len as f32);
        let (f, damp) = self.filter_coeffs(self.cfg.cutoff, self.cfg.reso, sr);
        let (wl, wh, wb) = match self.cfg.fkind { FK_HI => (0.0, 1.0, 0.0), FK_BAND => (0.0, 0.0, 1.0), _ => (1.0, 0.0, 0.0) };
        let scale = self.cfg.osc[0].level * (1.0 / 32767.0);
        let (vel, drive, post, pan_l, pan_r) = (self.vel, self.modulation[2], self.modulation[0], self.pan_l, self.pan_r);
        let drive_in = 1.0 + 8.0 * drive;
        let (mut flp, mut fbp, mut gain) = (self.flp, self.fbp, gain1);
        let slot = pid & 3;
        let tracked = if pid < 4 { 1.0 } else { 0.0 };
        for (n, frame) in chunk.iter_mut().enumerate() {
            gain += dgain;
            let index = (phase >> 22) as usize;
            let y0 = fm_table[index] as i32;
            let y = y0 + ((((fm_table[index + 1] as i32 - y0) as i64) * (phase & 0x3f_ffff) as i64) >> 22) as i32;
            phase = phase.wrapping_add(step);
            let x = y as f32 * scale;
            flp += f * fbp;
            let hp = x - flp - damp * fbp;
            fbp += f * hp;
            let mut v = (flp * wl + hp * wh + fbp * wb) * (gain * vel);
            if DRIVE { let t = v * drive_in; v += (t / (1.0 + t.abs()) - v) * drive; }
            let out = v * post;
            frame.0 += out * pan_l;
            frame.1 += out * pan_r;
            psum[n][slot] += out * tracked;
        }
        if !(flp.is_finite() && fbp.is_finite()) { flp = 0.0; fbp = 0.0; }
        self.flp = flp;
        self.fbp = fbp;
        self.phase[0][0] = phase as f32 * (1.0 / Q32);
        if self.gate_samples > 0 { self.gate_samples -= len as u32; }
        if !self.env[0].active() && !self.gate { self.active = false; }
        len as u32
    }

    fn set_modulation(&mut self, target: [f32; 3]) {
        self.modulation_target = target;
        #[cfg(any(feature = "rp32", all(target_os = "none", target_pointer_width = "32")))]
        { self.modulation_left = 0; }
        self.modulation_active = self.modulation != target;
    }

    fn advance_modulation(&mut self) {
        #[cfg(not(any(feature = "rp32", all(target_os = "none", target_pointer_width = "32"))))]
        {
            self.modulation_active = false;
            for i in 0..3 {
                let target = self.modulation_target[i];
                if self.modulation[i] != target {
                    self.modulation[i] += (target - self.modulation[i]) * (1.0 / 240.0);
                    if (self.modulation[i] - target).abs() < 0.00001 {
                        self.modulation[i] = target;
                    } else {
                        self.modulation_active = true;
                    }
                }
            }
            return;
        }
        #[cfg(any(feature = "rp32", all(target_os = "none", target_pointer_width = "32")))]
        {
            if self.modulation_left == 0 {
                self.modulation_active = false;
                for i in 0..3 {
                    let difference = self.modulation_target[i] - self.modulation[i];
                    if difference.abs() < 0.00001 {
                        self.modulation[i] = self.modulation_target[i];
                        self.modulation_delta[i] = 0.0;
                    } else {
                        self.modulation_delta[i] = difference * ((1.0 - 0.9353767) / 16.0);
                        self.modulation_active = true;
                    }
                }
                self.modulation_left = 16;
            }
            for i in 0..3 { self.modulation[i] += self.modulation_delta[i]; }
            self.modulation_left -= 1;
        }
    }

    #[inline(always)]
    fn tick(
        &mut self,
        sr: f32,
        dt: f32,
        #[cfg(any(feature = "rp32", all(target_os = "none", target_pointer_width = "32")))]
        fm_table: &[i16; 1025],
    ) -> f32 {
        if !self.active { return 0.0; }
        if self.modulation_active { self.advance_modulation(); }
        if self.gate_samples == 1 { self.note_off(); }
        if self.gate_samples > 0 { self.gate_samples -= 1; }

        let mut amp = 1.0f32;
        let mut have_amp = false;
        let mut cut_oct = 0.0f32;
        let mut pitch_semi = 0.0f32;
        let mut any = false;
        let mut amp_env = 1.0f32;
        for i in 0..2 {
            if !self.env_used[i] { continue; }
            let lv = self.env[i].tick(dt);
            if self.env[i].active() { any = true; }
            let ec = self.cfg.env[i];
            match ec.target {
                TGT_AMP | TGT_AMP_EXP => { amp *= lv; have_amp = true; amp_env = lv; self.kernel_gain = lv; }
                TGT_CUTOFF => cut_oct += (ec.depth / 100.0) * 6.0 * lv,
                TGT_PITCH => pitch_semi += ec.depth * lv,
                _ => {}
            }
        }
        if !have_amp { amp = 1.0; }
        if !any && !self.gate { self.active = false; return 0.0; }

        let (mut p_mul, mut cut_mul, mut amp_lfo) = (1.0f32, 1.0f32, 1.0f32);
        if self.lfo.enabled() {
            let m = self.lfo.tick(sr);
            match self.lfo.target {
                TGT_AMP => amp_lfo = (1.0 + m).max(0.0),
                TGT_CUTOFF => cut_mul = math::fast_exp2(m * 2.0),
                TGT_PITCH => p_mul = math::fast_exp2(m),
                _ => {}
            }
        }

        let freq = if pitch_semi == 0.0 && p_mul == 1.0 {
            self.base_freq
        } else {
            self.base_freq * (if pitch_semi != 0.0 { math::fast_exp2(pitch_semi / 12.0) } else { 1.0 }) * p_mul
        };
        let mut s = 0.0f32;
        let (car, modu) = (self.cfg.osc[0], self.cfg.osc[1]);
        let fm = matches!(modu.wave, WAVE_FM | WAVE_PREVIEW_FM) && modu.level > 0.0;
        if freq.to_bits() != self.step_freq.to_bits() || sr.to_bits() != self.step_sr.to_bits() {
            for u in 0..self.ucount {
                let uf = freq * self.uratio[u];
                self.phase_step[u] = [uf * car.mult / sr, uf * modu.mult / sr];
            }
            self.step_freq = freq;
            self.step_sr = sr;
        }
        for u in 0..self.ucount {
            let mut step = self.phase_step[u];
            if self.modulation[1] != 1.0 {
                step[0] *= self.modulation[1];
                step[1] *= self.modulation[1];
            }
            if fm {
                #[cfg(any(feature = "rp32", all(target_os = "none", target_pointer_width = "32")))]
                if self.baked_fm {
                    self.phase[u][0] = math::unit_remainder(self.phase[u][0] + step[0]);
                    if car.level > 0.0 {
                        s += synth_tables::sample(fm_table, self.phase[u][0]) * car.level;
                    }
                    continue;
                }
                {
                    self.phase[u][1] = math::unit_remainder(self.phase[u][1] + step[1]);
                    self.phase[u][0] = math::unit_remainder(self.phase[u][0] + step[0]);
                    if car.level > 0.0 {
                        let pm = if modu.wave == WAVE_PREVIEW_FM {
                            (1.0 - math::oscillator_sin(math::unit_remainder(self.phase[u][1] + 0.25)))
                                * modu.level * (0.5 * core::f32::consts::FRAC_1_PI)
                        } else {
                            math::oscillator_sin(self.phase[u][1]) * modu.level * core::f32::consts::FRAC_1_PI
                        };
                        s += wave_sample(car.wave, math::unit_euclid(self.phase[u][0] + pm), &mut self.lfsr) * car.level;
                    }
                    continue;
                }
            }
            for o in 0..2 {
                let oc = self.cfg.osc[o];
                if oc.level <= 0.0 { continue; }
                if oc.wave != 4 {
                    self.phase[u][o] = math::unit_remainder(self.phase[u][o] + step[o]);
                }
                s += wave_sample(oc.wave, self.phase[u][o], &mut self.lfsr) * oc.level;
            }
        }
        s /= self.ucount as f32;
        if !s.is_finite() {
            self.phase = [[0.0; 2]; MAX_UNISON];
            return 0.0;
        }

        let cutoff = if self.cfg.fkind == FK_LPG {
            self.cfg.cutoff * (0.05 + 0.95 * amp_env)
        } else if cut_oct == 0.0 && cut_mul == 1.0 {
            self.cfg.cutoff
        } else {
            self.cfg.cutoff * math::fast_exp2(cut_oct) * cut_mul
        };
        #[cfg(feature = "audio-profile")]
        super::profiling::mark(super::profiling::Stage::SynthFilter);
        s = self.filter(s, cutoff, self.cfg.reso, sr);
        s *= amp * self.vel * amp_lfo;
        #[cfg(feature = "audio-profile")]
        super::profiling::mark(super::profiling::Stage::SynthFx);
        let s = if self.cfg.fx_kind != 0 && !(self.cfg.fx_amt <= 0.0) { self.fx.process(s, sr) } else { s };
        let drive = self.modulation[2];
        let shaped = if drive > 0.0 {
            let x = s * (1.0 + 8.0 * drive);
            s + (x / (1.0 + x.abs()) - s) * drive
        } else { s };
        shaped * self.modulation[0]
    }
}

#[doc(hidden)]
pub static KERNEL_PROBE: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

pub struct Synth {
    modulation: [[f32; 3]; MAX_PATCHES],
    patches: [Patch; MAX_PATCHES],
    voices: Box<[Voice]>,
    #[cfg(any(feature = "rp32", all(target_os = "none", target_pointer_width = "32")))]
    fm_table: Box<[i16; 1025]>,
    #[cfg(any(feature = "rp32", all(target_os = "none", target_pointer_width = "32")))]
    fm_timbre: u8,
    #[cfg(any(feature = "rp32", all(target_os = "none", target_pointer_width = "32")))]
    fm_wave: u8,
    #[cfg(all(test, feature = "rp32"))]
    table_copies: usize,
    pool: usize,
    serial: u32,
    pid_peak: [f32; 4],
}

impl Synth {
    pub fn new() -> Self {
        Self {
            modulation: [[1.0, 1.0, 0.0]; MAX_PATCHES],
            patches: [Patch::default(); MAX_PATCHES],
            voices: vec![Voice::default(); MAX_VOICES].into_boxed_slice(),
            #[cfg(any(feature = "rp32", all(target_os = "none", target_pointer_width = "32")))]
            fm_table: Box::new(synth_tables::TABLES[0]),
            #[cfg(any(feature = "rp32", all(target_os = "none", target_pointer_width = "32")))]
            fm_timbre: 0,
            #[cfg(any(feature = "rp32", all(target_os = "none", target_pointer_width = "32")))]
            fm_wave: WAVE_FM,
            #[cfg(all(test, feature = "rp32"))]
            table_copies: 0,
            pool: 8,
            serial: 0,
            pid_peak: [0.0; 4],
        }
    }

    fn patch(&mut self, pid: usize) -> Option<&mut Patch> {
        self.patches.get_mut(pid)
    }

    pub fn osc(&mut self, pid: usize, idx: usize, wave: u8, semi: i32, fine: f32, level: f32) {
        #[cfg(any(feature = "rp32", all(target_os = "none", target_pointer_width = "32")))]
        let timbre = ((level * 100.0 - 10.0) * 0.125 + 0.5).clamp(0.0, 10.0) as u8;
        if let Some(p) = self.patch(pid) {
            if idx < 2 {
                p.osc[idx] = OscCfg {
                    wave,
                    #[cfg(any(
                        feature = "rp32",
                        all(target_os = "none", target_pointer_width = "32")
                    ))]
                    timbre,
                    mult: math::powf(2.0f32, (semi as f32 + fine / 100.0) / 12.0),
                    level,
                };
            }
        }
        #[cfg(any(feature = "rp32", all(target_os = "none", target_pointer_width = "32")))]
        if matches!(wave, WAVE_FM | WAVE_PREVIEW_FM)
            && (timbre != self.fm_timbre || wave != self.fm_wave)
        {
            *self.fm_table = if wave == WAVE_PREVIEW_FM {
                super::preview_tables::FM[timbre as usize]
            } else {
                synth_tables::TABLES[timbre as usize]
            };
            self.fm_wave = wave;
            #[cfg(all(test, feature = "rp32"))]
            {
                self.table_copies += 1;
            }
            self.fm_timbre = timbre;
        }
    }

    pub fn filter(&mut self, pid: usize, kind: u8, cutoff: f32, reso: f32) {
        if let Some(p) = self.patch(pid) {
            p.fkind = kind;
            p.cutoff = cutoff.max(20.0);
            p.reso = reso;
        }
    }

    pub fn env(&mut self, pid: usize, slot: usize, target: u8, depth: f32, atk: f32, dec: f32, sus: f32, rel: f32) {
        if let Some(p) = self.patch(pid) {
            if slot < 2 {
                p.env[slot] = EnvCfg { target, depth, atk, dec, sus, rel };
            }
        }
    }

    pub fn lfo(&mut self, pid: usize, target: u8, wave: u8, rate: f32, depth: f32) {
        if let Some(p) = self.patch(pid) {
            p.lfo.set(target, wave, rate, depth);
        }
    }

    pub fn pan(&mut self, pid: usize, pos: f32) {
        if let Some(p) = self.patch(pid) {
            p.pan = pos;
            if pos == 0.0 {
                p.pan_l = core::f32::consts::FRAC_1_SQRT_2;
                p.pan_r = core::f32::consts::FRAC_1_SQRT_2;
            } else {
                let a = (pos + 1.0) * core::f32::consts::FRAC_PI_4;
                p.pan_l = math::cos(a);
                p.pan_r = math::sin(a);
            }
        }
    }

    pub fn unison(&mut self, pid: usize, count: u8, detune: f32) {
        if let Some(p) = self.patch(pid) {
            p.unison = count;
            p.detune = detune;
        }
    }

    pub fn fx(&mut self, pid: usize, kind: u8, amt: f32, param: f32) {
        if let Some(p) = self.patch(pid) {
            p.fx_kind = kind;
            p.fx_amt = amt;
            p.fx_param = param;
        }
    }

    pub fn set_voices(&mut self, n: usize) {
        self.pool = n.clamp(1, MAX_VOICES);
    }

    /// Temporary live controls do not overwrite the stored patch or its envelopes.
    pub fn modulate(&mut self, pid: usize, gain: f32, pitch: f32, drive: f32) {
        if let Some(target) = self.modulation.get_mut(pid) {
            if gain.is_finite() && pitch.is_finite() && drive.is_finite() {
                let next = [gain.clamp(0.0, 1.0), pitch.clamp(0.5, 2.0), drive.clamp(0.0, 1.0)];
                if *target != next {
                    *target = next;
                    for voice in self.voices.iter_mut().filter(|v| v.active && v.pid == pid) {
                        voice.set_modulation(next);
                    }
                }
            }
        }
    }

    fn alloc(&self) -> usize {
        for i in 0..self.pool {
            if !self.voices[i].active { return i; }
        }
        let mut best = 0;
        let mut best_rank = 0;
        for i in 0..self.pool {
            if self.voices[i].rank >= best_rank { best_rank = self.voices[i].rank; best = i; }
        }
        best
    }

    #[cfg_attr(all(target_arch = "arm", target_os = "none"), inline(never))]
    pub fn note_on(&mut self, pid: usize, note: u8, vel: f32, gate_samples: u32) {
        if pid >= MAX_PATCHES { return; }
        self.serial = self.serial.wrapping_add(1);
        // Active ranks stay below MAX_VOICES; prefix counts close gaps left by finished voices.
        let mut ranks = [0u8; MAX_VOICES];
        for voice in &self.voices {
            if voice.active { ranks[voice.rank as usize] += 1; }
        }
        let mut lower = 0;
        for rank in &mut ranks {
            let count = *rank;
            *rank = lower;
            lower += count;
        }
        for voice in &mut self.voices {
            if voice.active { voice.rank = ranks[voice.rank as usize] as u32; }
        }
        let slot = self.alloc();
        let replaced = if self.voices[slot].active { self.voices[slot].rank } else { u32::MAX };
        for (i, voice) in self.voices.iter_mut().enumerate() {
            if i != slot && voice.active && voice.rank < replaced { voice.rank += 1; }
        }
        let p = self.patches[pid];
        self.voices[slot].start(&p, pid, note, vel, gate_samples, self.serial);
        self.voices[slot].modulation=self.modulation[pid];
        self.voices[slot].set_modulation(self.modulation[pid]);

    }

    pub fn note_off(&mut self, pid: usize, note: u8) {
        for i in 0..self.pool {
            let v = &mut self.voices[i];
            if v.active && v.gate && v.pid == pid && v.note == note { v.note_off(); }
        }
    }

    pub fn stop(&mut self, pid: usize) {
        for v in self.voices.iter_mut() {
            if v.active && v.pid == pid { v.note_off(); }
        }
    }

    pub fn panic(&mut self) {
        for v in self.voices.iter_mut() { v.active = false; }
    }

    #[cfg_attr(all(target_arch = "arm", target_os = "none"), unsafe(link_section = ".data.ram_func"))]
    #[cfg_attr(all(target_arch = "arm", target_os = "none"), inline(never))]
    pub fn tick(&mut self, sr: f32) -> (f32, f32) {
        let mut tl = 0.0f32;
        let mut tr = 0.0f32;
        let mut psum = [0.0f32; 4];
        #[cfg(feature = "audio-profile")]
        let (mut active, mut fm, mut lfo, mut bitcrush) = (0u32, 0u32, 0u32, 0u32);
        let dt = 1.0 / sr;
        for i in 0..self.pool {
            let voice = &mut self.voices[i];
            if !voice.active {
                continue;
            }
            #[cfg(feature = "audio-profile")]
            {
                active += 1;
                fm += u32::from(matches!(voice.cfg.osc[1].wave, WAVE_FM | WAVE_PREVIEW_FM) && voice.cfg.osc[1].level > 0.0);
                lfo += u32::from(voice.lfo.enabled());
                bitcrush += u32::from(voice.fx.kind == super::fx::FX_BITCRUSH);
                super::profiling::mark(super::profiling::Stage::SynthVoice);
            }
            let pid = voice.pid;
            #[cfg(any(feature = "rp32", all(target_os = "none", target_pointer_width = "32")))]
            let fm_table = if voice.cfg.osc[1].timbre == self.fm_timbre && voice.cfg.osc[1].wave == self.fm_wave {
                &*self.fm_table
            } else {
                // Old tails retain their timbre without evicting the current RAM table.
                if voice.cfg.osc[1].wave == WAVE_PREVIEW_FM {
                    &super::preview_tables::FM[voice.cfg.osc[1].timbre as usize]
                } else {
                    &synth_tables::TABLES[voice.cfg.osc[1].timbre as usize]
                }
            };
            let s = voice.tick(
                sr,
                dt,
                #[cfg(any(feature = "rp32", all(target_os = "none", target_pointer_width = "32")))]
                fm_table,
            );
            // equal-power pan from the voice's patch.
            #[cfg(feature = "audio-profile")]
            super::profiling::mark(super::profiling::Stage::SynthPan);
            tl += s * voice.pan_l;
            tr += s * voice.pan_r;
            if pid < 4 { psum[pid] += s; }
        }
        #[cfg(feature = "audio-profile")]
        super::profiling::voices(self.pool as u32, active, fm, lfo, bitcrush);
        for p in 0..4 {
            if psum[p].abs() > self.pid_peak[p] { self.pid_peak[p] = psum[p].abs(); }
        }
        (tl, tr)
    }

    /// Renders `out.len()` frames voice by voice (JUCE renderNextBlock style). Each frame sums
    /// the voices in the same order as `tick`, so the result is bit-identical.
    #[cfg_attr(all(target_arch = "arm", target_os = "none"), unsafe(link_section = ".data.ram_func"))]
    pub fn render(&mut self, sr: f32, out: &mut [(f32, f32)]) {
        const CHUNK: usize = 64;
        let dt = 1.0 / sr;
        for chunk in out.chunks_mut(CHUNK) {
            let mut psum = [[0.0f32; 4]; CHUNK];
            for frame in chunk.iter_mut() { *frame = (0.0, 0.0); }
            for i in 0..self.pool {
                let voice = &mut self.voices[i];
                if !voice.active { continue; }
                let pid = voice.pid;
                #[cfg(any(feature = "rp32", all(target_os = "none", target_pointer_width = "32")))]
                let fm_table = if voice.cfg.osc[1].timbre == self.fm_timbre && voice.cfg.osc[1].wave == self.fm_wave {
                    &*self.fm_table
                } else if voice.cfg.osc[1].wave == WAVE_PREVIEW_FM {
                    &super::preview_tables::FM[voice.cfg.osc[1].timbre as usize]
                } else {
                    &synth_tables::TABLES[voice.cfg.osc[1].timbre as usize]
                };
                #[cfg(feature = "audio-profile")]
                let (started, mut rendered) = (super::profiling::cycles(), 0u32);
                #[cfg(feature = "audio-profile")]
                { rendered = voice.render(sr, dt, #[cfg(any(feature = "rp32", all(target_os = "none", target_pointer_width = "32")))] fm_table, chunk, &mut psum, pid); }
                #[cfg(not(feature = "audio-profile"))]
                voice.render(sr, dt, #[cfg(any(feature = "rp32", all(target_os = "none", target_pointer_width = "32")))] fm_table, chunk, &mut psum, pid);
                #[cfg(feature = "audio-profile")]
                super::profiling::synth_work(rendered, super::profiling::cycles().wrapping_sub(started));
            }
            for sums in &psum[..chunk.len()] {
                for p in 0..4 {
                    if sums[p].abs() > self.pid_peak[p] { self.pid_peak[p] = sums[p].abs(); }
                }
            }
        }
    }

    pub fn take_pid_peaks(&mut self) -> [f32; 4] {
        let p = self.pid_peak;
        self.pid_peak = [0.0; 4];
        p
    }

    pub fn pid_voices(&self) -> [u32; 4] {
        let mut v = [0u32; 4];
        for i in 0..self.pool {
            let voice = &self.voices[i];
            if voice.active && voice.pid < 4 { v[voice.pid] += 1; }
        }
        v
    }
}

#[cfg(test)]
mod rank_tests {
    use super::*;
    // Frozen quadratic rank policy is an independent oracle for allocation and rendered output.
    fn reference_note_on(s: &mut Synth, note: u8) {
        s.serial = s.serial.wrapping_add(1);
        let ranks = core::array::from_fn::<_, MAX_VOICES, _>(|i| s.voices[i].rank);
        let active = core::array::from_fn::<_, MAX_VOICES, _>(|i| s.voices[i].active);
        for v in &mut s.voices {
            if v.active { v.rank = ranks.iter().zip(active).filter(|(r, live)| *live && **r < v.rank).count() as u32; }
        }
        let slot = s.alloc();
        let replaced = if s.voices[slot].active { s.voices[slot].rank } else { u32::MAX };
        for (i, v) in s.voices.iter_mut().enumerate() {
            if i != slot && v.active && v.rank < replaced { v.rank += 1; }
        }
        s.voices[slot].start(&s.patches[0], 0, note, 0.6, 0, s.serial);
        s.voices[slot].modulation = s.modulation[0];
        s.voices[slot].set_modulation(s.modulation[0]);
    }

    #[test]
    fn rank_compaction_preserves_stealing_completion_pool_changes_and_pcm() {
        for initial_pool in [1, 2, 8, MAX_VOICES] {
            for seed in [1u32, 17, 921, u32::MAX] {
                let mut actual = Synth::new();
                let mut expected = Synth::new();
                for s in [&mut actual, &mut expected] {
                    s.set_voices(initial_pool);
                    s.serial = u32::MAX - 16;
                    s.env(0, 0, TGT_AMP, 1.0, 0.001, 0.01, 0.5, 0.01);
                }
                let mut rng = seed;
                for event in 0..1024 {
                    rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
                    let slot = (rng >> 16) as usize % MAX_VOICES;
                    match event % 19 {
                        0 => { let pool = 1 + slot; actual.set_voices(pool); expected.set_voices(pool); }
                        1 | 2 => { actual.voices[slot].active = false; expected.voices[slot].active = false; }
                        3 => { actual.note_off(0, 60); expected.note_off(0, 60); }
                        4 if event % 57 == 4 => { actual.panic(); expected.panic(); }
                        _ => {
                            let note = 48 + (rng >> 24) as u8 % 36;
                            actual.note_on(0, note, 0.6, 0);
                            reference_note_on(&mut expected, note);
                        }
                    }
                    for (a, b) in actual.voices.iter().zip(expected.voices.iter()) {
                        assert_eq!((a.active,a.rank,a.note,a.gate),(b.active,b.rank,b.note,b.gate), "pool={initial_pool} seed={seed} event={event}");
                        if a.active { assert!(a.rank < MAX_VOICES as u32); }
                    }
                    for _ in 0..8 {
                        let a = actual.tick(48000.0);
                        let b = expected.tick(48000.0);
                        assert_eq!((a.0.to_bits(), a.1.to_bits()),(b.0.to_bits(), b.1.to_bits()));
                    }
                }
            }
        }
    }

    #[test]
    fn voice_pool_does_not_travel_on_the_startup_stack() {
        assert!(core::mem::size_of::<Synth>() < 8192);
    }

    #[test]
    fn centered_pan_is_bit_identical_in_both_channels() {
        for pan in [0.0, -0.0] {
            let mut synth = Synth::new();
            synth.pan(0, pan);
            synth.note_on(0, 60, 1.0, 0);
            let mut audible = false;
            for _ in 0..1024 {
                let (l, r) = synth.tick(48000.0);
                audible |= l != 0.0;
                assert_eq!(l.to_bits(), r.to_bits());
            }
            assert!(audible);
        }
    }

    #[test]
    fn sustained_voice_stays_oldest_after_finished_voice_reuse() {
        let mut synth = Synth::new(); synth.set_voices(2);
        synth.note_on(0, 60, 1.0, 0);
        for _ in 0..10000 {
            synth.note_on(0, 61, 1.0, 0);
            assert!(synth.voices.iter().filter(|v| v.active).all(|v| v.rank < MAX_VOICES as u32));
            synth.voices[1].active = false;
        }
        synth.note_on(0, 62, 1.0, 0);
        synth.note_on(0, 63, 1.0, 0);
        assert_eq!(synth.voices[0].note, 63);
        assert_eq!(synth.voices[1].note, 62);
    }
}

#[cfg(test)]
mod modulation_ramp_tests {
    use super::*;

    #[test]
    fn finite_control_segments_track_the_original_smoother_and_recover_exactly() {
        for interval in [7, 16, 33, 240, 960] {
            let mut voice = Voice::default();
            let mut reference = [1.0f32, 1.0, 0.0];
            let mut target = reference;
            for n in 0..16000 {
                if n % interval == 0 {
                    target = [[0.0, 0.78, 0.85], [0.4, 0.9, 0.4], [0.8, 0.95, 0.2], [1.0, 1.0, 0.0]][(n / interval) % 4];
                    voice.set_modulation(target);
                }
                let previous = voice.modulation;
                if voice.modulation_active { voice.advance_modulation(); }
                for i in 0..3 {
                    reference[i] += (target[i] - reference[i]) / 240.0;
                    if (reference[i] - target[i]).abs() < 0.00001 { reference[i] = target[i]; }
                    assert!((voice.modulation[i] - reference[i]).abs() < if interval == 7 { 0.0035 } else { 0.0006 });
                    assert!((voice.modulation[i] - previous[i]).abs() <= 1.0 / 240.0 + 0.00001);
                    assert!(voice.modulation[i] >= previous[i].min(target[i]) - 0.00001);
                    assert!(voice.modulation[i] <= previous[i].max(target[i]) + 0.00001);
                }
            }
            voice.set_modulation([1.0, 1.0, 0.0]);
            for _ in 0..8000 { if voice.modulation_active { voice.advance_modulation(); } }
            assert_eq!(voice.modulation, [1.0, 1.0, 0.0]);
            assert!(!voice.modulation_active);
        }
    }

    #[test]
    fn pitch_modulation_does_not_invalidate_unmodulated_phase_steps() {
        for sr in [22050.0, 44100.0, 48000.0, 96000.0] {
            for unison in [1, 3, 7] {
                let mut synth = Synth::new();
                synth.set_voices(1);
                synth.unison(0, unison, 7.0);
                synth.note_on(0, 60, 0.5, 0);
                synth.tick(sr);
                let steps = synth.voices[0].phase_step;
                synth.modulate(0, 1.0, 0.78, 0.0);
                let mut phase_error = 0.0f64;
                for _ in 0..4000 {
                    let old_phase = synth.voices[0].phase;
                    synth.tick(sr);
                    let voice = &synth.voices[0];
                    assert_eq!(voice.phase_step, steps);
                    for u in 0..unison as usize {
                        let expected = old_phase[u][0] + steps[u][0] * voice.modulation[1];
                        let expected = expected - libm::floorf(expected);
                        phase_error += (expected - voice.phase[u][0]).abs() as f64;
                    }
                }
                assert!(phase_error < 0.0001);
            }
        }
    }
}

#[cfg(test)]
mod filter_prepare_tests {
    use super::*;

    #[test]
    fn prepared_filter_matches_original_through_modulation_retrigger_and_rate_changes() {
        for kind in [0, FK_HI, FK_BAND, FK_LPG] {
            for mode in 0..4 {
                for cutoff in [20.0, 6400.0, 18000.0] {
                    let mut patch = Patch::default();
                    patch.fkind = kind;
                    patch.cutoff = cutoff;
                    patch.reso = 70.0;
                    patch.env[0].rel = 0.025;
                    if mode == 1 { patch.env[1] = EnvCfg { target: TGT_CUTOFF, depth: 60.0, atk: 0.01, dec: 0.02, sus: 0.4, rel: 0.02 }; }
                    if mode == 2 { patch.lfo.set(TGT_CUTOFF, 0, 4.0, 0.7); }
                    if mode == 3 { patch.lfo.set(TGT_PITCH, 0, 4.0, 0.7); }
                    let mut candidate = Voice::default();
                    let mut original = Voice::default();
                    for n in 0..6000 {
                        if n == 0 || n == 3000 {
                            candidate.start(&patch, 0, 60, 0.8, 0, 17);
                            original.start(&patch, 0, 60, 0.8, 0, 17);
                            original.fixed_filter = false;
                        }
                        if n == 1800 || n == 4500 { candidate.note_off(); original.note_off(); }
                        let sr = [48000.0, 22050.0, 96000.0][(n / 700) % 3];
                        let a = candidate.tick(
                            sr,
                            1.0 / sr,
                            #[cfg(any(feature = "rp32", all(target_os = "none", target_pointer_width = "32")))]
                            &synth_tables::TABLES[0],
                        );
                        let b = original.tick(
                            sr,
                            1.0 / sr,
                            #[cfg(any(feature = "rp32", all(target_os = "none", target_pointer_width = "32")))]
                            &synth_tables::TABLES[0],
                        );
                        assert_eq!(a.to_bits(), b.to_bits(), "kind={kind} mode={mode} cutoff={cutoff} n={n}");
                        assert_eq!(candidate.active, original.active);
                    }
                }
            }
        }
    }

    #[test]
    fn fm_wavetable_matches_device_oscillator_across_all_timbres_and_phases() {
        for t in 0..=10 {
            let level = (10 + 8 * t) as f32 / 100.0;
            for i in 0..4096 {
                let p0 = i as f32 / 4096.0;
                let pm = math::phase_sin_lut(math::unit_remainder(p0 * 2.0)) * level * core::f32::consts::FRAC_1_PI;
                let expected = math::phase_sin_lut(math::unit_euclid(p0 + pm));
                assert!((synth_tables::sample(&synth_tables::TABLES[t], p0) - expected).abs() < 6e-4, "t={t} i={i}");
            }
        }
    }
}

#[cfg(all(test, feature = "rp32"))]
mod fm_overlap_tests {
    use super::*;
    fn configure(s: &mut Synth, timbre: u8, wave: u8) {
        s.osc(0, 0, 1, 0, 0.0, 0.5);
        s.osc(0, 1, wave, 12, 0.0, (10 + 8 * timbre) as f32 / 100.0);
    }
    #[test]
    fn generic_fm_preserves_carrier_ratio_depth_and_unison() {
        for (wave, ratio, level, unison) in [
            (2, 2.0, 0.42, 1), (3, 2.0, 0.42, 1),
            (1, 1.0, 0.42, 1), (1, 3.0, 0.42, 1),
            (1, 2.0, 0.4, 1), (1, 2.0, 1.5, 1),
            (1, 2.0, 0.42, 2),
        ] {
            for mode in [WAVE_FM, WAVE_PREVIEW_FM] {
                let mut patch = Patch::default();
                patch.osc[0] = OscCfg { wave, mult: 1.0, level: 0.5, timbre: 0 };
                patch.osc[1] = OscCfg { wave: mode, mult: ratio, level, timbre: 4 };
                patch.unison = unison;
                let mut voice = Voice::default();
                voice.start(&patch, 0, 60, 1.0, 0, 1);
                assert!(!voice.baked_fm);
                let mut filter = Voice::default();
                for _ in 0..128 {
                    let actual = voice.tick(48000.0, 1.0 / 48000.0, &synth_tables::TABLES[4]);
                    let mut raw = 0.0;
                    for phase in &voice.phase[..voice.ucount] {
                        let pm = if mode == WAVE_PREVIEW_FM {
                            (1.0 - math::oscillator_sin(math::unit_remainder(phase[1] + 0.25)))
                                * level * (0.5 * core::f32::consts::FRAC_1_PI)
                        } else {
                            math::oscillator_sin(phase[1]) * level * core::f32::consts::FRAC_1_PI
                        };
                        raw += wave_sample(wave, math::unit_euclid(phase[0] + pm), &mut 1) * 0.5;
                    }
                    let expected = filter.filter(raw / voice.ucount as f32, patch.cutoff, patch.reso, 48000.0);
                    assert_eq!(actual, expected, "wave={wave} ratio={ratio} level={level} unison={unison} mode={mode}");
                }
            }
        }
    }

    #[test]
    fn overlapping_timbres_match_independent_voices_without_render_time_table_copies() {
        for (old_wave, new_wave) in [
            (WAVE_FM, WAVE_FM),
            (WAVE_PREVIEW_FM, WAVE_PREVIEW_FM),
            (WAVE_FM, WAVE_PREVIEW_FM),
            (WAVE_PREVIEW_FM, WAVE_FM),
        ] {
            for old in 0..=10 {
                for new in 0..=10 {
                    for count in [2, 4, 8] {
                        let mut mixed = Synth::new();
                        let mut isolated = alloc::vec::Vec::new();
                        for i in 0..count {
                            let timbre = if i % 2 == 0 { old } else { new };
                            let note = 36 + i as u8 * 3;
                            let wave = if i % 2 == 0 { old_wave } else { new_wave };
                            configure(&mut mixed, timbre, wave);
                            mixed.note_on(0, note, 0.5, 0);
                            assert!(mixed.voices[i].baked_fm);
                            let mut single = Synth::new();
                            configure(&mut single, timbre, wave);
                            single.note_on(0, note, 0.5, 0);
                            isolated.push(single);
                        }
                        let copies = mixed.table_copies;
                        for _ in 0..192 {
                            let expected = isolated
                                .iter_mut()
                                .map(|s| s.tick(48_000.0))
                                .fold((0.0, 0.0), |(l, r), (a, b)| (l + a, r + b));
                            assert_eq!(
                                mixed.tick(48_000.0),
                                expected,
                                "old={old} new={new} voices={count}"
                            );
                            assert_eq!(
                                mixed.table_copies, copies,
                                "render copied a table: old={old} new={new} voices={count}"
                            );
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod preview_tests {
    use super::*;

    fn html_frequency_fm(phase: f32, tone: usize) -> f32 {
        let theta = phase as f64 * core::f64::consts::TAU;
        let beta = 0.1 + 0.08 * tone as f64;
        // Integral of f + f*(.2+1.6*tone/10)*sin(4πft), from zero phase.
        (theta + beta * (1.0 - (2.0 * theta).cos())).sin() as f32
    }

    #[test]
    fn baked_preview_wave_matches_integrated_frequency_not_generic_phase_fm() {
        let mut separates_old_pm = false;
        for tone in 0..=10 {
            for i in 0..16384 {
                let phase = i as f32 / 16384.0;
                let actual = synth_tables::sample(&super::super::preview_tables::FM[tone], phase);
                let expected = html_frequency_fm(phase, tone);
                assert!(
                    (actual - expected).abs() < 1e-4,
                    "tone={tone} phase={phase}"
                );
                let theta = phase as f64 * core::f64::consts::TAU;
                let wrong = (theta + (0.1 + 0.08 * tone as f64) * (2.0 * theta).sin()).sin() as f32;
                separates_old_pm |= (actual - wrong).abs() > 0.1;
            }
        }
        assert!(separates_old_pm);
    }

    #[test]
    fn preview_dry_voices_follow_html_wave_across_rates_gates_and_retrigger() {
        for sr in [8000.0, 22050.0, 44100.0, 48000.0, 96000.0] {
            for tone in 0..=10 {
                for note in [36, 60, 84] {
                    let end = 0.15 + 0.04 * tone as f32;
                    let mut patch = Patch::default();
                    patch.osc[0].wave = 1;
                    patch.osc[1].wave = WAVE_PREVIEW_FM;
                    patch.osc[1].level = 0.1 + 0.08 * tone as f32;
                    patch.osc[1].mult = 2.0;
                    #[cfg(any(
                        feature = "rp32",
                        all(target_os = "none", target_pointer_width = "32")
                    ))]
                    {
                        patch.osc[1].timbre = tone as u8;
                    }
                    patch.cutoff = 400.0 + 60.0 * (tone * tone) as f32;
                    patch.env[0] = EnvCfg {
                        target: TGT_AMP_EXP,
                        depth: 100.0,
                        atk: 0.01,
                        dec: end - 0.01,
                        sus: 0.0,
                        rel: end,
                    };
                    let mut voice = Voice::default();
                    let mut unchanged_filter = Voice::default();
                    let mut envelope = Adsr::default();
                    for n in 0..4096 {
                        if n == 0 || n == 2048 {
                            voice.start(&patch, 0, note, 0.35, 1024, 1);
                            unchanged_filter = Voice::default();
                            envelope.set_exponential(0.01, end - 0.01, 0.0, end);
                            envelope.gate_on();
                        }
                        if n % 2048 == 1023 {
                            envelope.gate_off();
                        }
                        let amplitude = envelope.tick(1.0 / sr);
                        #[cfg(any(
                            feature = "rp32",
                            all(target_os = "none", target_pointer_width = "32")
                        ))]
                        let actual = voice.tick(sr, 1.0 / sr, &super::super::preview_tables::FM[tone]);
                        #[cfg(not(any(
                            feature = "rp32",
                            all(target_os = "none", target_pointer_width = "32")
                        )))]
                        let actual = voice.tick(sr, 1.0 / sr);
                        let raw = html_frequency_fm(voice.phase[0][0], tone);
                        let expected = if voice.active {
                            unchanged_filter.filter(raw, patch.cutoff, 0.0, sr) * amplitude * 0.35
                        } else {
                            0.0
                        };
                        assert!(actual.is_finite());
                        assert!(
                            (actual - expected).abs() < 2e-4,
                            "sr={sr} tone={tone} note={note} n={n}: {actual} != {expected}"
                        );
                    }
                }
            }
        }
    }
}
