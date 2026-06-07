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
