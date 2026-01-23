use silt_lua::{Compiler, ExVal, Lua};

#[allow(unused_macros)]
#[macro_export]
macro_rules! valeq {
    ($source:expr, $val:expr) => {
        assert_eq!(
            simple($source),
            $val.into(),
            "output does not match expected value"
        );
    };
}

#[allow(unused_macros)]
#[macro_export]
macro_rules! test_number {
    ($name:ident, $source:literal, $expected:expr) => {
        #[test]
        fn $name() {
            valeq!($source, ExVal::Number($expected));
        }
    };
}

#[allow(unused_macros)]
#[macro_export]
macro_rules! test_string {
    ($name:ident, $source:literal, $expected:literal) => {
        #[test]
        fn $name() {
            valeq!($source, ExVal::String($expected.to_string()));
        }
    };
}

#[allow(unused_macros)]
#[macro_export]
macro_rules! test_bool {
    ($name:ident, $source:literal, $expected:expr) => {
        #[test]
        fn $name() {
            valeq!($source, ExVal::Bool($expected));
        }
    };
}

#[allow(unused_macros)]
#[macro_export]
macro_rules! test_nil {
    ($name:ident, $source:literal) => {
        #[test]
        fn $name() {
            valeq!($source, ExVal::Nil);
        }
    };
}

pub fn simple(source: &str) -> ExVal {
    let mut compiler = Compiler::new();
    let mut lua = Lua::new_with_standard();
    match lua.run(source, &mut compiler) {
        Ok(v) => v,
        Err(e) => ExVal::String(e[0].to_string()),
    }
}

pub fn run_lua(source: &str) -> Result<ExVal, Vec<String>> {
    let mut compiler = Compiler::new();
    let mut lua = Lua::new_with_standard();
    lua.run(source, &mut compiler)
        .map_err(|e| e.iter().map(|err| err.to_string()).collect())
}
