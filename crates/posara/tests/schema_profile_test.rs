#![cfg(feature = "compiler")]
use abrase::{compiler::Compiler, lexer::Lexer, parser::Parser};

#[test]
fn native_schema_compiles_both_profiles_without_constructing_a_host() {
    let source = "pub fn probe() -> <IO> Int { now() } fn main() -> Int { 0 }";
    let mut parser = Parser::new(Lexer::new(source));
    let declarations = parser.parse_program();
    assert!(parser.errors.is_empty());
    for narrow in [false, true] {
        let mut compiler = Compiler::new().with_int32_mode(narrow);
        posara::schema::register_host_fns(&mut compiler).unwrap();
        let module = compiler.compile_module(&declarations).unwrap();
        assert_eq!(module.flags & polka::CART_FLAG_INT32_SAFE != 0, narrow);
        macro_rules! run {
            ($profile:ident) => {{
                let host = posara_core::$profile::Host::new(posara_core::$profile::Drivers::null());
                let mut vm = myriad::$profile::VirtualMachine::new();
                host.install(&mut vm);
                assert_eq!(vm.call_export(&module, "probe", &[]).unwrap().as_int(), 0);
            }};
        }
        if narrow { run!(word32) } else { run!(word64) }
    }
}
