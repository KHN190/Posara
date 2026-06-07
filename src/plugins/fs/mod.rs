use std::cell::RefCell;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};
use std::rc::Rc;

use myriad::{alloc_string, read_string, NativeCtx, Value, VirtualMachine};

const F_READ: i64 = 1;
const F_WRITE: i64 = 2;
const F_CREATE: i64 = 4;
const F_APPEND: i64 = 8;
const F_TRUNC: i64 = 16;

#[derive(Default)]
pub struct FdTable {
    files: Vec<Option<File>>,
}

impl FdTable {
    pub fn new() -> Self {
        Self { files: Vec::new() }
    }

    fn insert(&mut self, f: File) -> i64 {
        if let Some(i) = self.files.iter().position(|s| s.is_none()) {
            self.files[i] = Some(f);
            i as i64
        } else {
            self.files.push(Some(f));
            (self.files.len() - 1) as i64
        }
    }

    fn get(&mut self, fd: i64) -> Option<&mut File> {
        usize::try_from(fd).ok().and_then(|i| self.files.get_mut(i)).and_then(|s| s.as_mut())
    }

    fn close(&mut self, fd: i64) -> bool {
        match usize::try_from(fd).ok().and_then(|i| self.files.get_mut(i)) {
            Some(slot @ Some(_)) => { *slot = None; true }
            _ => false,
        }
    }
}

// Join `rel` under `root`, rejecting absolute paths and any `..` escape.
// Lexical only (no canonicalize) so paths that don't exist yet still resolve.
pub fn resolve(root: &Path, rel: &str) -> Option<PathBuf> {
    let mut out = root.to_path_buf();
    for comp in Path::new(rel).components() {
        match comp {
            Component::Normal(c) => out.push(c),
            Component::CurDir => {}
            _ => return None,
        }
    }
    Some(out)
}

fn arg(args: &[Value], i: usize) -> i64 {
    args.get(i).copied().unwrap_or(Value::ZERO).as_int()
}

fn path_arg(ctx: &NativeCtx, root: &Path, args: &[Value], i: usize) -> Option<PathBuf> {
    let s = read_string(ctx.heap, *args.get(i)?)?;
    resolve(root, &s)
}

fn ret_int(n: i64) -> Result<(Value, bool), String> { Ok((Value::from_int(n), false)) }

pub fn register_natives(vm: &mut VirtualMachine, root: PathBuf, fds: Rc<RefCell<FdTable>>) {
    let r = root.clone();
    vm.register_native("fs_exists", Rc::new(move |ctx: &mut NativeCtx, a: &[Value]| {
        let ok = path_arg(ctx, &r, a, 0).map(|p| p.exists()).unwrap_or(false);
        ret_int(ok as i64)
    }));
    let r = root.clone();
    vm.register_native("fs_mkdir", Rc::new(move |ctx: &mut NativeCtx, a: &[Value]| {
        let ok = path_arg(ctx, &r, a, 0).map(|p| std::fs::create_dir_all(p).is_ok()).unwrap_or(false);
        ret_int(if ok { 0 } else { -1 })
    }));
    let r = root.clone();
    vm.register_native("fs_remove", Rc::new(move |ctx: &mut NativeCtx, a: &[Value]| {
        let ok = path_arg(ctx, &r, a, 0).map(|p| {
            if p.is_dir() { std::fs::remove_dir_all(p).is_ok() } else { std::fs::remove_file(p).is_ok() }
        }).unwrap_or(false);
        ret_int(if ok { 0 } else { -1 })
    }));
    let r = root.clone();
    vm.register_native("fs_list", Rc::new(move |ctx: &mut NativeCtx, a: &[Value]| {
        let entries = path_arg(ctx, &r, a, 0)
            .and_then(|p| std::fs::read_dir(p).ok())
            .map(|rd| {
                let mut names: Vec<String> = rd.filter_map(|e| e.ok())
                    .filter_map(|e| e.file_name().into_string().ok())
                    .collect();
                names.sort();
                names.join("\n")
            })
            .unwrap_or_default();
        Ok((alloc_string(ctx.heap, &entries)?, true))
    }));

    let r = root.clone();
    let fdt = Rc::clone(&fds);
    vm.register_native("fs_open", Rc::new(move |ctx: &mut NativeCtx, a: &[Value]| {
        let Some(path) = path_arg(ctx, &r, a, 0) else { return ret_int(-1); };
        let flags = arg(a, 1);
        let mut o = OpenOptions::new();
        o.read(flags & F_READ != 0)
            .write(flags & (F_WRITE | F_APPEND) != 0)
            .create(flags & F_CREATE != 0)
            .append(flags & F_APPEND != 0)
            .truncate(flags & F_TRUNC != 0 && flags & F_APPEND == 0);
        match o.open(&path) {
            Ok(f) => ret_int(fdt.borrow_mut().insert(f)),
            Err(e) => {
                eprintln!("• fs_open: {} ({e})", path.display());
                ret_int(-1)
            }
        }
    }));
    let fdt = Rc::clone(&fds);
    vm.register_native("fs_close", Rc::new(move |_: &mut NativeCtx, a: &[Value]| {
        ret_int(if fdt.borrow_mut().close(arg(a, 0)) { 0 } else { -1 })
    }));
    let fdt = Rc::clone(&fds);
    vm.register_native("fs_seek", Rc::new(move |_: &mut NativeCtx, a: &[Value]| {
        let (fd, off, whence) = (arg(a, 0), arg(a, 1), arg(a, 2));
        let mut t = fdt.borrow_mut();
        let Some(f) = t.get(fd) else { return ret_int(-1); };
        let from = match whence {
            0 => SeekFrom::Start(off.max(0) as u64),
            1 => SeekFrom::Current(off),
            2 => SeekFrom::End(off),
            _ => return ret_int(-1),
        };
        match f.seek(from) { Ok(p) => ret_int(p as i64), Err(_) => ret_int(-1) }
    }));

    // Read up to `n` bytes; returns Array<Int> of exactly `n` (zero-padded on
    // short read) so the cart can index 0..n safely (arrays have no len op).
    let fdt = Rc::clone(&fds);
    vm.register_native("fs_read", Rc::new(move |ctx: &mut NativeCtx, a: &[Value]| {
        let (fd, n) = (arg(a, 0), arg(a, 1).max(0) as usize);
        let mut buf = vec![0u8; n];
        let got = {
            let mut t = fdt.borrow_mut();
            match t.get(fd) { Some(f) => f.read(&mut buf).unwrap_or(0), None => return ret_int(-1) }
        };
        let (slot, gen_) = ctx.heap.try_alloc(n.max(1))?;
        let cells = ctx.heap.cell_data_mut(slot, gen_)?;
        for i in 0..n { cells[i] = buf.get(i).copied().unwrap_or(0) as u64; }
        let _ = got;
        Ok((Value::from_handle(slot, gen_), true))
    }));
    let fdt = Rc::clone(&fds);
    vm.register_native("fs_read_text", Rc::new(move |ctx: &mut NativeCtx, a: &[Value]| {
        let (fd, n) = (arg(a, 0), arg(a, 1).max(0) as usize);
        let mut buf = vec![0u8; n];
        let got = {
            let mut t = fdt.borrow_mut();
            match t.get(fd) { Some(f) => f.read(&mut buf).unwrap_or(0), None => return ret_int(-1) }
        };
        let s = String::from_utf8_lossy(&buf[..got]);
        Ok((alloc_string(ctx.heap, &s)?, true))
    }));

    let fdt = Rc::clone(&fds);
    vm.register_native("fs_write", Rc::new(move |ctx: &mut NativeCtx, a: &[Value]| {
        let fd = arg(a, 0);
        let data = a.get(1).copied().unwrap_or(Value::NONE);
        if data.is_handle_none() { return ret_int(-1); }
        let (slot, gen_) = data.as_handle();
        let bytes: Vec<u8> = ctx.heap.cell_data(slot, gen_)?.iter().map(|&w| w as u8).collect();
        let mut t = fdt.borrow_mut();
        let Some(f) = t.get(fd) else { return ret_int(-1); };
        match f.write(&bytes) { Ok(w) => ret_int(w as i64), Err(_) => ret_int(-1) }
    }));
    let fdt = Rc::clone(&fds);
    vm.register_native("fs_write_text", Rc::new(move |ctx: &mut NativeCtx, a: &[Value]| {
        let fd = arg(a, 0);
        let Some(s) = a.get(1).and_then(|v| read_string(ctx.heap, *v)) else { return ret_int(-1); };
        let mut t = fdt.borrow_mut();
        let Some(f) = t.get(fd) else { return ret_int(-1); };
        match f.write(s.as_bytes()) { Ok(w) => ret_int(w as i64), Err(_) => ret_int(-1) }
    }));
}

#[cfg(feature = "compiler")]
pub fn host_fn_decls() -> Vec<(&'static str, Vec<abrase::ty::Type>, abrase::ty::Type)> {
    use abrase::ty::Type as T;
    let arr_int = || T::Generic { name: "Array".into(), args: vec![T::Int] };
    let str_ty = || T::String;
    vec![
        ("fs_exists",     vec![str_ty()],                 T::Int),
        ("fs_mkdir",      vec![str_ty()],                 T::Int),
        ("fs_remove",     vec![str_ty()],                 T::Int),
        ("fs_list",       vec![str_ty()],                 T::String),
        ("fs_open",       vec![str_ty(), T::Int],         T::Int),
        ("fs_close",      vec![T::Int],                   T::Int),
        ("fs_seek",       vec![T::Int, T::Int, T::Int],   T::Int),
        ("fs_read",       vec![T::Int, T::Int],           arr_int()),
        ("fs_read_text",  vec![T::Int, T::Int],           T::String),
        ("fs_write",      vec![T::Int, arr_int()],        T::Int),
        ("fs_write_text", vec![T::Int, T::String],        T::Int),
    ]
}

pub struct FsPlugin {
    root: PathBuf,
    pub fds: Rc<RefCell<FdTable>>,
}

impl FsPlugin {
    pub fn new(root: PathBuf) -> Self {
        Self { root, fds: Rc::new(RefCell::new(FdTable::new())) }
    }
}

impl crate::plugin::Plugin for FsPlugin {
    fn install(&self, vm: &mut VirtualMachine) {
        register_natives(vm, self.root.clone(), Rc::clone(&self.fds));
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
