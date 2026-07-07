use polka::{BytecodeChunk, Chunk, Export, Module, NativeChunk, OpCode, Register};
use posara::lint::{lint_module, PosaraLint};

fn has(warns: &[PosaraLint], code: &str) -> bool {
    warns.iter().any(|w| w.code == code)
}

fn module_with(functions: Vec<Chunk>, exports: Vec<Export>) -> Module {
    Module { functions, entry: 0, exports, ..Default::default() }
}

fn bc() -> BytecodeChunk { BytecodeChunk::default() }
fn screen_native() -> Chunk { Chunk::Native(NativeChunk { name: "screen".into(), param_count: 2 }) }
fn cls_native()    -> Chunk { Chunk::Native(NativeChunk { name: "cls".into(),    param_count: 1 }) }
fn update_export(fn_id: u16) -> Export { Export { name: "update".into(), fn_id } }

// ── PosaraLint struct ─────────────────────────────────────────────────────────

#[test]
fn pretty_print_with_line() {
    let w = PosaraLint::new("missing_commit_frame", "msg").with_line(7);
    let s = w.pretty_print();
    assert!(s.contains("line 7") && s.contains("missing_commit_frame"));
}

#[test]
fn pretty_print_no_line() {
    let w = PosaraLint::new("device_port_out_of_range", "msg");
    let s = w.pretty_print();
    assert!(!s.contains("line") && s.contains("device_port_out_of_range"));
}

// ── clean module ──────────────────────────────────────────────────────────────

#[test]
fn clean_module_no_warns() {
    assert!(lint_module(&module_with(vec![], vec![])).is_empty());
}

// ── missing_commit_frame ──────────────────────────────────────────────────────

#[test]
fn missing_commit_frame_fires() {
    let mut chunk = bc();
    chunk.constants.extend([320u64, 240]);
    chunk.code.push(OpCode::PushConst(Register(0), 0));
    chunk.code.push(OpCode::PushConst(Register(1), 1));
    chunk.code.push(OpCode::Call(Register(2), 0));

    let m = module_with(vec![screen_native(), Chunk::Bytecode(chunk)], vec![update_export(1)]);
    assert!(has(&lint_module(&m), "missing_commit_frame"));
}

#[test]
fn missing_commit_frame_suppressed_when_committed() {
    let mut chunk = bc();
    chunk.constants.extend([320u64, 240, 0x2001, 1]);
    chunk.code.push(OpCode::PushConst(Register(0), 0));
    chunk.code.push(OpCode::PushConst(Register(1), 1));
    chunk.code.push(OpCode::Call(Register(2), 0));
    chunk.code.push(OpCode::PushConst(Register(3), 2));
    chunk.code.push(OpCode::PushConst(Register(4), 3));
    chunk.code.push(OpCode::Deo(Register(4), Register(3)));

    let m = module_with(vec![screen_native(), Chunk::Bytecode(chunk)], vec![update_export(1)]);
    assert!(!has(&lint_module(&m), "missing_commit_frame"));
}

#[test]
fn missing_commit_frame_suppressed_by_commit_native() {
    // update() that calls screen() then the commit() native — no port write.
    let mut chunk = bc();
    chunk.constants.extend([320u64, 240]);
    chunk.code.push(OpCode::PushConst(Register(0), 0));
    chunk.code.push(OpCode::PushConst(Register(1), 1));
    chunk.code.push(OpCode::Call(Register(2), 0)); // screen (fn 0)
    chunk.code.push(OpCode::Call(Register(2), 1)); // commit (fn 1)

    let commit_native = Chunk::Native(NativeChunk { name: "commit".into(), param_count: 0 });
    let m = module_with(vec![screen_native(), commit_native, Chunk::Bytecode(chunk)], vec![update_export(2)]);
    assert!(!has(&lint_module(&m), "missing_commit_frame"));
}

#[test]
fn missing_commit_frame_no_update_no_warn() {
    let mut chunk = bc();
    chunk.constants.extend([320u64, 240]);
    chunk.code.push(OpCode::PushConst(Register(0), 0));
    chunk.code.push(OpCode::PushConst(Register(1), 1));
    chunk.code.push(OpCode::Call(Register(2), 0));

    let m = module_with(vec![screen_native(), Chunk::Bytecode(chunk)], vec![]);
    assert!(!has(&lint_module(&m), "missing_commit_frame"));
}

// ── device_port_out_of_range ──────────────────────────────────────────────────

#[test]
fn device_port_out_of_range_deo_fires() {
    let mut chunk = bc();
    chunk.constants.extend([0xDEADu64, 0u64]);
    chunk.code.push(OpCode::PushConst(Register(0), 0));
    chunk.code.push(OpCode::PushConst(Register(1), 1));
    chunk.code.push(OpCode::Deo(Register(1), Register(0)));

    let m = module_with(vec![Chunk::Bytecode(chunk)], vec![]);
    assert!(has(&lint_module(&m), "device_port_out_of_range"));
}

#[test]
fn device_port_out_of_range_dei_fires() {
    let mut chunk = bc();
    chunk.constants.extend([0xDEADu64, 0u64]);
    chunk.code.push(OpCode::PushConst(Register(0), 0));
    chunk.code.push(OpCode::PushConst(Register(1), 1));
    chunk.code.push(OpCode::Dei(Register(1), Register(0)));

    let m = module_with(vec![Chunk::Bytecode(chunk)], vec![]);
    assert!(has(&lint_module(&m), "device_port_out_of_range"));
}

#[test]
fn known_controller_port_no_warn() {
    let mut chunk = bc();
    chunk.constants.extend([0x8002u64, 0u64]);
    chunk.code.push(OpCode::PushConst(Register(0), 0));
    chunk.code.push(OpCode::PushConst(Register(1), 1));
    chunk.code.push(OpCode::Dei(Register(1), Register(0)));

    let m = module_with(vec![Chunk::Bytecode(chunk)], vec![]);
    assert!(!has(&lint_module(&m), "device_port_out_of_range"));
}

#[test]
fn known_screen_port_no_warn() {
    let mut chunk = bc();
    chunk.constants.extend([0x2001u64, 1u64]);
    chunk.code.push(OpCode::PushConst(Register(0), 0));
    chunk.code.push(OpCode::PushConst(Register(1), 1));
    chunk.code.push(OpCode::Deo(Register(1), Register(0)));

    let m = module_with(vec![Chunk::Bytecode(chunk)], vec![]);
    assert!(!has(&lint_module(&m), "device_port_out_of_range"));
}

// ── draw_without_screen ───────────────────────────────────────────────────────

#[test]
fn draw_without_screen_fires() {
    let mut chunk = bc();
    chunk.code.push(OpCode::Call(Register(0), 0));

    let m = module_with(vec![cls_native(), Chunk::Bytecode(chunk)], vec![]);
    assert!(has(&lint_module(&m), "draw_without_screen"));
}

#[test]
fn draw_with_screen_no_warn() {
    let mut chunk = bc();
    chunk.constants.extend([320u64, 240, 0x2001u64, 1]);
    chunk.code.push(OpCode::PushConst(Register(0), 0));
    chunk.code.push(OpCode::PushConst(Register(1), 1));
    chunk.code.push(OpCode::Call(Register(2), 0));
    chunk.code.push(OpCode::Call(Register(3), 1));
    chunk.code.push(OpCode::PushConst(Register(4), 2));
    chunk.code.push(OpCode::PushConst(Register(5), 3));
    chunk.code.push(OpCode::Deo(Register(5), Register(4)));

    let m = module_with(
        vec![screen_native(), cls_native(), Chunk::Bytecode(chunk)],
        vec![update_export(2)],
    );
    assert!(!has(&lint_module(&m), "draw_without_screen"));
}

// ── screen_in_update ──────────────────────────────────────────────────────────

#[test]
fn screen_in_update_fires() {
    let mut chunk = bc();
    chunk.constants.extend([320u64, 240, 0x2001u64, 1]);
    chunk.code.push(OpCode::PushConst(Register(0), 0));
    chunk.code.push(OpCode::PushConst(Register(1), 1));
    chunk.code.push(OpCode::Call(Register(2), 0));
    chunk.code.push(OpCode::PushConst(Register(3), 2));
    chunk.code.push(OpCode::PushConst(Register(4), 3));
    chunk.code.push(OpCode::Deo(Register(4), Register(3)));

    let m = module_with(vec![screen_native(), Chunk::Bytecode(chunk)], vec![update_export(1)]);
    assert!(has(&lint_module(&m), "screen_in_update"));
}

#[test]
fn screen_outside_update_no_screen_in_update_warn() {
    let mut start = bc();
    start.constants.extend([320u64, 240]);
    start.code.push(OpCode::PushConst(Register(0), 0));
    start.code.push(OpCode::PushConst(Register(1), 1));
    start.code.push(OpCode::Call(Register(2), 0));

    let mut upd = bc();
    upd.constants.extend([0x2001u64, 1]);
    upd.code.push(OpCode::PushConst(Register(0), 0));
    upd.code.push(OpCode::PushConst(Register(1), 1));
    upd.code.push(OpCode::Deo(Register(1), Register(0)));

    let m = module_with(
        vec![screen_native(), Chunk::Bytecode(start), Chunk::Bytecode(upd)],
        vec![update_export(2)],
    );
    assert!(!has(&lint_module(&m), "screen_in_update"));
}

// ── screen_multi_call ─────────────────────────────────────────────────────────

#[test]
fn screen_multi_call_fires() {
    let mut chunk = bc();
    chunk.constants.extend([320u64, 240, 0x2001u64, 1]);
    chunk.code.push(OpCode::PushConst(Register(0), 0));
    chunk.code.push(OpCode::PushConst(Register(1), 1));
    chunk.code.push(OpCode::Call(Register(2), 0));
    chunk.code.push(OpCode::Call(Register(3), 0));
    chunk.code.push(OpCode::PushConst(Register(4), 2));
    chunk.code.push(OpCode::PushConst(Register(5), 3));
    chunk.code.push(OpCode::Deo(Register(5), Register(4)));

    let m = module_with(vec![screen_native(), Chunk::Bytecode(chunk)], vec![update_export(1)]);
    assert!(has(&lint_module(&m), "screen_multi_call"));
}

#[test]
fn screen_called_once_no_multi_warn() {
    let mut chunk = bc();
    chunk.constants.extend([320u64, 240, 0x2001u64, 1]);
    chunk.code.push(OpCode::PushConst(Register(0), 0));
    chunk.code.push(OpCode::PushConst(Register(1), 1));
    chunk.code.push(OpCode::Call(Register(2), 0));
    chunk.code.push(OpCode::PushConst(Register(3), 2));
    chunk.code.push(OpCode::PushConst(Register(4), 3));
    chunk.code.push(OpCode::Deo(Register(4), Register(3)));

    let m = module_with(vec![screen_native(), Chunk::Bytecode(chunk)], vec![update_export(1)]);
    assert!(!has(&lint_module(&m), "screen_multi_call"));
}

// ── #4: effect-dispatch ABI ports are known (regression: real carts emit
//        0xE100/0xE200 for every effectful call — must NOT be flagged) ─────────

#[test]
fn effect_dispatch_ports_not_flagged() {
    for port in [0xE100u64, 0xE101, 0xE102, 0xE200] {
        let mut chunk = bc();
        chunk.constants.extend([port, 0u64]);
        chunk.code.push(OpCode::PushConst(Register(0), 0));
        chunk.code.push(OpCode::PushConst(Register(1), 1));
        chunk.code.push(OpCode::Deo(Register(1), Register(0)));
        let m = module_with(vec![Chunk::Bytecode(chunk)], vec![]);
        assert!(!has(&lint_module(&m), "device_port_out_of_range"), "port {port:#06x} wrongly flagged");
    }
}

// ── #4: const propagation across calls / arithmetic / branches ────────────────

#[test]
fn port_const_survives_call() {
    // port loaded, a Call intervenes, then Deo — caller regs survive a call,
    // so the bad port must still be flagged.
    let mut chunk = bc();
    chunk.constants.extend([0xDEADu64, 0u64]);
    chunk.code.push(OpCode::PushConst(Register(0), 0));
    chunk.code.push(OpCode::PushConst(Register(1), 1));
    chunk.code.push(OpCode::Call(Register(5), 0));
    chunk.code.push(OpCode::Deo(Register(1), Register(0)));

    let m = module_with(vec![cls_native(), Chunk::Bytecode(chunk)], vec![]);
    assert!(has(&lint_module(&m), "device_port_out_of_range"));
}

#[test]
fn call_clobbers_only_dest_reg() {
    // a Call whose dest IS the port reg invalidates it → cannot prove port → silent.
    let mut chunk = bc();
    chunk.constants.extend([0xDEADu64, 0u64]);
    chunk.code.push(OpCode::PushConst(Register(0), 0));
    chunk.code.push(OpCode::PushConst(Register(1), 1));
    chunk.code.push(OpCode::Call(Register(0), 0));
    chunk.code.push(OpCode::Deo(Register(1), Register(0)));

    let m = module_with(vec![cls_native(), Chunk::Bytecode(chunk)], vec![]);
    assert!(!has(&lint_module(&m), "device_port_out_of_range"));
}

#[test]
fn port_via_addimm_fires() {
    let mut chunk = bc();
    chunk.constants.extend([0xDE00u64, 0u64]);
    chunk.code.push(OpCode::PushConst(Register(0), 0));
    chunk.code.push(OpCode::AddImm(Register(0), Register(0), 5));
    chunk.code.push(OpCode::PushConst(Register(1), 1));
    chunk.code.push(OpCode::Deo(Register(1), Register(0)));

    let m = module_with(vec![Chunk::Bytecode(chunk)], vec![]);
    assert!(has(&lint_module(&m), "device_port_out_of_range"));
}

#[test]
fn branch_clears_const_no_false_positive() {
    // const set before a branch must not leak past it (soundness over recall).
    let mut chunk = bc();
    chunk.constants.extend([0xDEADu64, 0u64]);
    chunk.code.push(OpCode::PushConst(Register(0), 0));
    chunk.code.push(OpCode::PushConst(Register(1), 1));
    chunk.code.push(OpCode::Jz(Register(1), 1));
    chunk.code.push(OpCode::Deo(Register(1), Register(0)));

    let m = module_with(vec![Chunk::Bytecode(chunk)], vec![]);
    assert!(!has(&lint_module(&m), "device_port_out_of_range"));
}
