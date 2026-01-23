use silt_lua::{Compiler, ExVal, Lua};

#[allow(unused_macros)]
macro_rules! valeq {
    ($source:literal, $val:expr) => {
        assert_eq!(
            simple($source),
            $val.into(),
            "output does not match expected value"
        );
    };
}

fn simple(source: &str) -> ExVal {
    let mut compiler = Compiler::new();
    let mut lua = Lua::new_with_standard();
    match lua.run(source, &mut compiler) {
        Ok(v) => v,
        Err(e) => ExVal::String(e[0].to_string()),
    }
}

#[test]
fn vararg() {
    valeq!(
        r#"
        function test(...)
            local a=...
            return a
        end
        return test(1,3,7,12)
        "#,
        1
    );
}
