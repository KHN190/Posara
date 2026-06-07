// Cart-facing synth API: thin natives pushing Cmds onto the sfx engine's ring.
// The voice machine itself lives in plugins/sfx/synth.rs.

use std::rc::Rc;
use std::sync::Arc;

use myriad::{NativeCtx, Value, VirtualMachine};

use crate::plugin::Plugin;
use crate::plugins::sfx::mixer::Cmd;
use crate::plugins::sfx::output::{push, CmdProd};

pub struct SynthPlugin {
    cmds: CmdProd,
}

impl SynthPlugin {
    pub fn new(cmds: CmdProd) -> Self {
        Self { cmds }
    }
}

impl Plugin for SynthPlugin {
    fn install(&self, vm: &mut VirtualMachine) {
        register_natives(vm, Arc::clone(&self.cmds));
    }

    #[cfg(feature = "compiler")]
    fn register_fns(&self, compiler: &mut abrase::compiler::Compiler) -> Result<(), String> {
        use abrase::ast::EffectItem;
        let io_eff = || vec![EffectItem { name: vec!["IO".into()], arg: None }];
        for (name, params, ret) in host_fn_decls() {
            compiler.register_host_fn(name, params, ret, io_eff())?;
        }
        Ok(())
    }
}

fn arg(args: &[Value], i: usize) -> i64 {
    args.get(i).copied().unwrap_or(Value::ZERO).as_int()
}

fn ret_unit() -> Result<(Value, bool), String> { Ok((Value::UNIT, false)) }

pub fn register_natives(vm: &mut VirtualMachine, cmds: CmdProd) {
    macro_rules! native {
        ($name:literal, |$a:ident| $cmd:expr) => {{
            let p = Arc::clone(&cmds);
            vm.register_native($name, Rc::new(move |_ctx: &mut NativeCtx, $a: &[Value]| {
                push(&p, $cmd);
                ret_unit()
            }));
        }};
    }

    native!("synth_osc", |a| Cmd::SynOsc(arg(a, 0).max(0) as usize, arg(a, 1).max(0) as usize,
        arg(a, 2).clamp(0, 4) as u8, arg(a, 3), arg(a, 4) as f32, (arg(a, 5).clamp(0, 100) as f32) / 100.0));
    native!("synth_filter", |a| Cmd::SynFilter(arg(a, 0).max(0) as usize, arg(a, 1).clamp(0, 3) as u8,
        arg(a, 2).max(0) as f32, arg(a, 3) as f32));
    native!("synth_env", |a| Cmd::SynEnv(arg(a, 0).max(0) as usize, arg(a, 1).max(0) as usize,
        arg(a, 2).clamp(0, 2) as u8, arg(a, 3) as f32, arg(a, 4).max(0) as f32 / 1000.0,
        arg(a, 5).max(0) as f32 / 1000.0, (arg(a, 6).clamp(0, 100) as f32) / 100.0, arg(a, 7).max(0) as f32 / 1000.0));
    native!("synth_lfo", |a| Cmd::SynLfo(arg(a, 0).max(0) as usize, arg(a, 1).clamp(0, 2) as u8,
        arg(a, 2).max(0) as f32 / 100.0, (arg(a, 3).clamp(0, 100) as f32) / 100.0));
    native!("synth_unison", |a| Cmd::SynUnison(arg(a, 0).max(0) as usize, arg(a, 1).clamp(1, 7) as u8, arg(a, 2) as f32));
    native!("synth_fx", |a| Cmd::SynFx(arg(a, 0).max(0) as usize, arg(a, 1).clamp(0, 5) as u8,
        (arg(a, 2).clamp(0, 100) as f32) / 100.0, arg(a, 3).max(0) as f32));
    native!("synth_voices", |a| Cmd::SynVoices(arg(a, 0).max(1) as usize));
    native!("synth_on", |a| Cmd::SynOn(arg(a, 0).max(0) as usize, arg(a, 1).clamp(0, 127) as u8,
        (arg(a, 2).clamp(0, 100) as f32) / 100.0, arg(a, 3).max(0) as u32));
    native!("synth_off", |a| Cmd::SynOff(arg(a, 0).max(0) as usize, arg(a, 1).clamp(0, 127) as u8));
    native!("synth_stop", |a| Cmd::SynStop(arg(a, 0).max(0) as usize));
    native!("synth_panic", |_a| Cmd::SynPanic);
}

#[cfg(feature = "compiler")]
pub fn host_fn_decls() -> Vec<(&'static str, Vec<abrase::ty::Type>, abrase::ty::Type)> {
    use abrase::ty::Type as T;
    vec![
        ("synth_osc",    vec![T::Int, T::Int, T::Int, T::Int, T::Int, T::Int],         T::Unit),
        ("synth_filter", vec![T::Int, T::Int, T::Int, T::Int],                         T::Unit),
        ("synth_env",    vec![T::Int, T::Int, T::Int, T::Int, T::Int, T::Int, T::Int, T::Int], T::Unit),
        ("synth_lfo",    vec![T::Int, T::Int, T::Int, T::Int],                         T::Unit),
        ("synth_unison", vec![T::Int, T::Int, T::Int],                                 T::Unit),
        ("synth_fx",     vec![T::Int, T::Int, T::Int, T::Int],                         T::Unit),
        ("synth_voices", vec![T::Int],                                                 T::Unit),
        ("synth_on",     vec![T::Int, T::Int, T::Int, T::Int],                         T::Unit),
        ("synth_off",    vec![T::Int, T::Int],                                         T::Unit),
        ("synth_stop",   vec![T::Int],                                                 T::Unit),
        ("synth_panic",  vec![],                                                       T::Unit),
    ]
}
