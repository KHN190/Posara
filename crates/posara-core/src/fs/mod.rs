pub mod storage;

pub use storage::{MemStorage, Storage};

use crate::prelude::*;

use myriad::{alloc_bytes, alloc_string, read_string, NativeCtx, Value, VirtualMachine};

use crate::abi::Args;


fn path_arg(ctx: &NativeCtx, args: &[Value], i: usize) -> Option<String> {
    read_string(ctx.heap, Args(args).handle(i))
}

fn ret_int(n: i64) -> Result<(Value, bool), String> { Ok((Value::from_int(n), false)) }

const MAX_READ: usize = 1 << 26;

fn read_len(a: &[Value], operation: &str) -> Result<usize, String> {
    Args(a).count(1, MAX_READ).map_err(|e| format!("{operation}: {e} bytes"))
}

pub fn register_natives(vm: &mut VirtualMachine, storage: Rc<dyn Storage>) {
    let s = Rc::clone(&storage);
    vm.register_native("fs_exists", Rc::new(move |ctx: &mut NativeCtx, a: &[Value]| {
        ret_int(path_arg(ctx, a, 0).map(|p| s.exists(&p)).unwrap_or(false) as i64)
    }));
    let s = Rc::clone(&storage);
    vm.register_native("fs_mkdir", Rc::new(move |ctx: &mut NativeCtx, a: &[Value]| {
        ret_int(if path_arg(ctx, a, 0).map(|p| s.mkdir(&p)).unwrap_or(false) { 0 } else { -1 })
    }));
    let s = Rc::clone(&storage);
    vm.register_native("fs_remove", Rc::new(move |ctx: &mut NativeCtx, a: &[Value]| {
        ret_int(if path_arg(ctx, a, 0).map(|p| s.remove(&p)).unwrap_or(false) { 0 } else { -1 })
    }));
    let s = Rc::clone(&storage);
    vm.register_native("fs_list", Rc::new(move |ctx: &mut NativeCtx, a: &[Value]| {
        let entries = path_arg(ctx, a, 0).map(|p| s.list(&p).join("\n")).unwrap_or_default();
        Ok((alloc_string(ctx.heap, &entries)?, true))
    }));

    let s = Rc::clone(&storage);
    vm.register_native("fs_open", Rc::new(move |ctx: &mut NativeCtx, a: &[Value]| {
        let Some(path) = path_arg(ctx, a, 0) else { return ret_int(-1); };
        ret_int(s.open(&path, Args(a).word(1)))
    }));
    let s = Rc::clone(&storage);
    vm.register_native("fs_close", Rc::new(move |_: &mut NativeCtx, a: &[Value]| {
        ret_int(if s.close(Args(a).word(0)) { 0 } else { -1 })
    }));
    let s = Rc::clone(&storage);
    vm.register_native("fs_seek", Rc::new(move |_: &mut NativeCtx, a: &[Value]| {
        let a = Args(a);
        ret_int(s.seek(a.word(0), a.word(2), a.offset(1)))
    }));

    for (name, text) in [("fs_read", false), ("fs_reads", true)] {
        let storage = Rc::clone(&storage);
        vm.register_native(name, Rc::new(move |ctx: &mut NativeCtx, args: &[Value]| {
            let result = (|| {
                let fd = Args(args).word(0);
                let n = read_len(args, name)?;
                let mut bytes = storage.read(fd, n).ok_or_else(|| format!("{name}: failed to read fd {fd}"))?;
                if text { alloc_string(ctx.heap, &String::from_utf8_lossy(&bytes)) }
                else { bytes.resize(n, 0); alloc_bytes(ctx.heap, &bytes) }
            })();
            crate::abi::read_result(ctx, result)
        }));
    }

    let s = Rc::clone(&storage);
    vm.register_native("fs_write", Rc::new(move |ctx: &mut NativeCtx, a: &[Value]| {
        let fd = Args(a).word(0);
        let Some(bytes) = myriad::read_bytes(ctx.heap, Args(a).handle(1)) else { return ret_int(-1); };
        ret_int(s.write(fd, &bytes).map(|w| w as i64).unwrap_or(-1))
    }));
    let s = Rc::clone(&storage);
    vm.register_native("fs_writes", Rc::new(move |ctx: &mut NativeCtx, a: &[Value]| {
        let fd = Args(a).word(0);
        let Some(text) = read_string(ctx.heap, Args(a).handle(1)) else { return ret_int(-1); };
        ret_int(s.write(fd, text.as_bytes()).map(|w| w as i64).unwrap_or(-1))
    }));
}

pub struct FsPlugin {
    pub storage: Rc<dyn Storage>,
}

impl FsPlugin {
    pub fn new(storage: Rc<dyn Storage>) -> Self {
        Self { storage }
    }
}

impl crate::plugin::Plugin for FsPlugin {
    fn install(&self, vm: &mut VirtualMachine) {
        register_natives(vm, Rc::clone(&self.storage));
    }
}
