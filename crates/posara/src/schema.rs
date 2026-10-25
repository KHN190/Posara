pub fn register_host_fns(compiler: &mut abrase::compiler::Compiler) -> Result<(), String> {
    use abrase::ast::EffectItem;
    use abrase::ty::Type as T;
    use posara_core::abi::{Cap, Eff, Ty, NATIVES};

    fn enabled(cap: Cap) -> bool {
        match cap {
            Cap::Core => true,
            Cap::Gfx | Cap::Input => cfg!(feature = "gfx"),
            Cap::GfxFs => cfg!(all(feature = "gfx", feature = "fs")),
            Cap::Sfx => cfg!(feature = "sfx"),
            Cap::SfxFs => cfg!(all(feature = "sfx", feature = "fs")),
            Cap::Synth => cfg!(feature = "synth"),
            Cap::Fs => cfg!(feature = "fs"),
            Cap::Midi => cfg!(feature = "midi"),
            Cap::Net => cfg!(feature = "net"),
            Cap::Imu => cfg!(feature = "imu"),
        }
    }
    fn ty(t: Ty) -> T {
        let bytes = || T::Named("Bytes".into());
        let arr_int = || T::Generic { name: "Array".into(), args: vec![T::Int] };
        match t {
            Ty::Int => T::Int,
            Ty::Float => T::Float,
            Ty::Bool => T::Bool,
            Ty::Str => T::String,
            Ty::Unit => T::Unit,
            Ty::Bytes => bytes(),
            Ty::RefBytes => T::Reference { is_mut: false, inner: Box::new(bytes()) },
            Ty::ArrInt => arr_int(),
            Ty::RefArrInt => T::Reference { is_mut: false, inner: Box::new(arr_int()) },
        }
    }
    fn eff(e: Eff) -> Vec<EffectItem> {
        let name = match e {
            Eff::Pure => return vec![],
            Eff::Graphics => "Graphics",
            Eff::IO => "IO",
            Eff::IOExnString => return vec![
                EffectItem { name: vec!["IO".into()], arg: None },
                EffectItem { name: vec!["exn".into()], arg: Some(Box::new(abrase::ast::Type::Named("String".into()))) },
            ],
            Eff::Network => "net",
            Eff::Nondet => "nondet",
        };
        vec![EffectItem { name: vec![name.into()], arg: None }]
    }

    for s in NATIVES.iter().filter(|s| enabled(s.cap)) {
        compiler.register_host_fn(s.name, s.params.iter().map(|&t| ty(t)).collect(), ty(s.ret), eff(s.eff))?;
    }
    Ok(())
}
