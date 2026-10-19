pub mod storage;

pub use storage::{MemStorage, Storage};

use crate::prelude::*;

use myriad::{alloc_bytes, alloc_string, read_string, NativeCtx, Value, VirtualMachine};


fn arg(args: &[Value], i: usize) -> i64 {
    args.get(i).copied().unwrap_or(Value::ZERO).as_int()
}

fn path_arg(ctx: &NativeCtx, args: &[Value], i: usize) -> Option<String> {
    read_string(ctx.heap, *args.get(i)?)
}

fn ret_int(n: i64) -> Result<(Value, bool), String> { Ok((Value::from_int(n), false)) }

const MAX_READ: usize = 1 << 26;

fn read_len(a: &[Value]) -> Result<usize, String> {
    let n = arg(a, 1).max(0) as usize;
    if n > MAX_READ { return Err(format!("fs_read: {n} bytes is over the {MAX_READ} limit")); }
    Ok(n)
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
        ret_int(s.open(&path, arg(a, 1)))
    }));
    let s = Rc::clone(&storage);
    vm.register_native("fs_close", Rc::new(move |_: &mut NativeCtx, a: &[Value]| {
        ret_int(if s.close(arg(a, 0)) { 0 } else { -1 })
    }));
    let s = Rc::clone(&storage);
    vm.register_native("fs_seek", Rc::new(move |_: &mut NativeCtx, a: &[Value]| {
        ret_int(s.seek(arg(a, 0), arg(a, 2), arg(a, 1)))
    }));

    // Read up to `n` bytes into a packed Bytes value, zero-padded past EOF.
    let s = Rc::clone(&storage);
    vm.register_native("fs_read", Rc::new(move |ctx: &mut NativeCtx, a: &[Value]| {
        let (fd, n) = (arg(a, 0), read_len(a)?);
        let Some(mut buf) = s.read(fd, n) else { return ret_int(-1); };
        buf.resize(n, 0);
        Ok((alloc_bytes(ctx.heap, &buf)?, true))
    }));
    let s = Rc::clone(&storage);
    vm.register_native("fs_reads", Rc::new(move |ctx: &mut NativeCtx, a: &[Value]| {
        let (fd, n) = (arg(a, 0), read_len(a)?);
        let Some(buf) = s.read(fd, n) else { return ret_int(-1); };
        Ok((alloc_string(ctx.heap, &String::from_utf8_lossy(&buf))?, true))
    }));

    let s = Rc::clone(&storage);
    vm.register_native("fs_write", Rc::new(move |ctx: &mut NativeCtx, a: &[Value]| {
        let fd = arg(a, 0);
        let data = a.get(1).copied().unwrap_or(Value::NONE);
        if data.is_handle_none() { return ret_int(-1); }
        let (slot, gen_) = data.as_handle();
        let bytes: Vec<u8> = ctx.heap.cell_data(slot, gen_)?.iter().map(|&w| w as u8).collect();
        ret_int(s.write(fd, &bytes).map(|w| w as i64).unwrap_or(-1))
    }));
    let s = Rc::clone(&storage);
    vm.register_native("fs_writes", Rc::new(move |ctx: &mut NativeCtx, a: &[Value]| {
        let fd = arg(a, 0);
        let Some(text) = a.get(1).and_then(|v| read_string(ctx.heap, *v)) else { return ret_int(-1); };
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
