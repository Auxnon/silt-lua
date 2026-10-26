// With the implicit-return flag on (as the `silt` command and REPL run), a
// function returns its trailing expression — and nothing else: declarations and
// nested function definitions at the end of a body return nil.

use silt_lua::{Compiler, ExVal, Lua};

fn run(src: &str) -> ExVal {
    let mut lua = Lua::new_with_standard();
    let mut compiler = Compiler::new_with_flags(true, true, false);
    lua.run(None, src, &mut compiler).unwrap_or_else(|e| panic!("{:?}", e))
}

#[test]
fn trailing_expression_is_returned() {
    assert_eq!(run("function f() 1 + 2 end return f()"), ExVal::Integer(3));
}

#[test]
fn trailing_local_returns_nil() {
    assert_eq!(run("function f() local x = 1 end return f()"), ExVal::Nil);
    assert_eq!(run("function f() local x, y = 1, 2 end return f()"), ExVal::Nil);
}

#[test]
fn trailing_nested_function_returns_nil() {
    let src = r#"
        function b()
          function c()
            return 4
          end
        end
        return b()
    "#;
    assert_eq!(run(src), ExVal::Nil);
    let src = r#"
        function b()
          local function c() 4 end
        end
        return b()
    "#;
    assert_eq!(run(src), ExVal::Nil);
}

#[test]
fn nested_global_function_is_defined_by_the_call() {
    let src = r#"
        function b()
          function c()
            return 4
          end
        end
        b()
        return c()
    "#;
    assert_eq!(run(src), ExVal::Integer(4));
}

#[test]
fn nested_global_function_after_a_local() {
    // DEFINE_GLOBAL used to read the root's constants, so `c` resolved to the
    // wrong slot (here a number: "Constant store corrupted")
    let src = r#"
        function b()
          local q = 1
          function c() return q + 3 end
        end
        b()
        return c()
    "#;
    assert_eq!(run(src), ExVal::Integer(4));
}

#[test]
fn block_statements_are_not_returned() {
    // a trailing if/else at the root used to panic (its last POP was dropped)
    let src = r#"
        x = 0
        if x then
          print("yes")
        else
          print("no")
        end
    "#;
    run(src);
    let src = r#"
        function f(n)
          if n > 0 then
            print("pos")
          else
            print("neg")
          end
        end
        return f(1)
    "#;
    assert_eq!(run(src), ExVal::Nil);
    assert_eq!(run("function f() for i = 1, 2 do print(i) end end return f()"), ExVal::Nil);
    assert_eq!(run("function f() while false do print(1) end end return f()"), ExVal::Nil);
}
