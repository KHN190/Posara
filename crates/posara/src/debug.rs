use myriad::{DebugEvent, DebugSink};
use myriad::debug::render_fn_label;

pub fn register_assert_natives(vm: &mut myriad::VirtualMachine) {
    use std::rc::Rc;
    use myriad::{NativeCtx, Value, read_string};
    vm.register_native("assert", Rc::new(|ctx: &mut NativeCtx, a: &[Value]| {
        if a.first().is_some_and(|v| v.as_bool()) {
            return Ok((Value::UNIT, false));
        }
        let msg = a.get(1).and_then(|v| read_string(ctx.heap, *v)).unwrap_or_default();
        Err(format!("assert failed: {msg}"))
    }));
}

#[derive(Clone, Copy, Default)]
pub struct DebugCfg {
    pub trace: bool,
    pub handlers: bool,
    pub leak: bool,
}

impl DebugCfg {
    pub fn needs_sink(&self) -> bool {
        self.trace || self.handlers || break_at_spec().is_some()
    }
}

// BREAK_AT=<fn-name|#id>:<pc> — host-side breakpoint; on the matching trace
// event, dump the fn's register window with handle annotations.
fn break_at_spec() -> Option<(String, usize)> {
    let spec = std::env::var("BREAK_AT").ok()?;
    let parsed = parse_break_spec(&spec);
    if parsed.is_none() {
        eprintln!("BREAK_AT: expected <fn>:<pc>, got '{spec}'");
    }
    parsed
}

pub fn parse_break_spec(spec: &str) -> Option<(String, usize)> {
    let (f, p) = spec.rsplit_once(':')?;
    Some((f.to_string(), p.parse().ok()?))
}

pub fn break_matches(fn_part: &str, want_pc: usize, func: usize, pc: usize, names: &[String]) -> bool {
    pc == want_pc && fn_part.strip_prefix('#')
        .map(|id| id.parse::<usize>() == Ok(func))
        .unwrap_or_else(|| names.get(func).is_some_and(|n| n == fn_part))
}

fn dump_break(func: usize, pc: usize, op: &polka::OpCode, base_reg: usize, window: &[u64], handle_mask: u128, names: &[String]) {
    eprintln!("[break {}:{}] {:?} (base r{})", render_fn_label(func, names), pc, op, base_reg);
    for (i, raw) in window.iter().enumerate() {
        let is_h = i < 128 && (handle_mask & (1u128 << i)) != 0;
        if *raw == polka::HANDLE_NONE && !is_h { continue; }
        eprintln!("    r{i:<3} = {raw:#018x}{}", if is_h { "  (handle)" } else { "" });
    }
}

pub fn sink(trace: bool, handlers: bool) -> DebugSink {
    let break_at = break_at_spec();
    Box::new(move |event, names| match event {
        DebugEvent::Trace { func, pc, op, base_reg, window, handle_mask, file, line } => {
            if trace {
                if *line > 0 {
                    eprintln!("[{}:{} {}:{}] {:?}", render_fn_label(*func, names), pc, file, line, op);
                } else {
                    eprintln!("[{}:{}] {:?}", render_fn_label(*func, names), pc, op);
                }
            }
            if let Some((fn_part, want_pc)) = &break_at {
                if break_matches(fn_part, *want_pc, *func, *pc, names) {
                    dump_break(*func, *pc, op, *base_reg, window, *handle_mask, names);
                }
            }
        }
        DebugEvent::HandlePush { effect_id, suspend_pc, dest, depth, .. } if handlers => {
            eprintln!("  [handle] push effect_id={effect_id} suspend_pc={suspend_pc} dest=r{dest} depth={depth}");
        }
        DebugEvent::Resume { saved_pc, val, alive, depth, .. } if handlers => {
            eprintln!("  [resume] saved_pc={saved_pc} val={val:?} alive={alive:?} depth={depth}");
        }
        _ => {}
    })
}

#[cfg(feature = "compiler")]
pub fn disasm(path: &std::path::Path, host: &crate::Host) -> Result<(), String> {
    let r = crate::runner::compile_abe(path, host)?;
    let (module, static_by_offset, names) = (r.module, r.static_names, r.fn_names);
    for (i, chunk) in module.functions.iter().enumerate() {
        let entry = if i == module.entry { " <entry>" } else { "" };
        let name = names.get(i).cloned().unwrap_or_default();
        let is_module_init = name == "__module_init";
        match chunk {
            polka::Chunk::Bytecode(bc) => {
                println!("fn #{i} {name}{entry} (regs={}, consts={})", bc.reg_count, bc.constants.len());
                for (j, c) in bc.constants.iter().enumerate() {
                    println!("  const[{j}] = {c}");
                }
                for (pc, op) in bc.code.iter().enumerate() {
                    let ann = if is_module_init {
                        static_init_annotation(op, &static_by_offset)
                    } else {
                        call_annotation(op, &names)
                    };
                    if ann.is_empty() {
                        println!("  {pc:>4}: {op:?}");
                    } else {
                        println!("  {pc:>4}: {:<50}  ; {ann}", format!("{op:?}"));
                    }
                }
            }
            polka::Chunk::Native(n) => {
                println!("fn #{i} {name}{entry} <native, params={}>", n.param_count);
            }
        }
    }
    if !static_by_offset.is_empty() {
        println!("\nstatic table (offset -> name):");
        for (offset, name) in static_by_offset.iter().enumerate() {
            if !name.is_empty() {
                println!("  [{offset}] {name}");
            }
        }
    }
    Ok(())
}

#[cfg(not(feature = "compiler"))]
pub fn disasm(_path: &std::path::Path, _host: &crate::Host) -> Result<(), String> {
    Err("posara built without `compiler` feature".into())
}

#[cfg(feature = "compiler")]
fn call_annotation(op: &polka::OpCode, names: &[String]) -> String {
    use polka::OpCode;
    match op {
        OpCode::Call(_, id) => {
            let idx = *id as usize;
            match names.get(idx) {
                Some(n) if !n.is_empty() => format!("-> {n}#{idx}"),
                _ => format!("-> #{idx}"),
            }
        }
        _ => String::new(),
    }
}

#[cfg(feature = "compiler")]
fn static_init_annotation(op: &polka::OpCode, static_by_offset: &[String]) -> String {
    use polka::OpCode;
    match op {
        OpCode::St(_, _, off) | OpCode::Ld(_, _, off) => {
            let idx = *off as usize;
            match static_by_offset.get(idx) {
                Some(name) if !name.is_empty() => {
                    let arrow = if matches!(op, OpCode::St(..)) { "=" } else { "=>" };
                    format!("static[{idx}] {arrow} {name}")
                }
                _ => String::new(),
            }
        }
        _ => String::new(),
    }
}
