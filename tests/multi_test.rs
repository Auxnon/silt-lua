use silt_lua::{Compiler, ExVal, Lua};

fn simple(source: &str) -> ExVal {
    let mut compiler = Compiler::new();
    let mut lua = Lua::new_with_standard();
    match lua.run(source, &mut compiler) {
        Ok(v) => v,
        Err(e) => ExVal::String(e[0].to_string()),
    }
}



#[test]
fn mult() {
        assert_eq!(1,2)
}

#[test]
fn multiple_returns() {
    let source_in = r#"
        function get_values()
            return 2, "hello", true
        end
        
        local a, b, c = get_values()
        return {a=a, b=b, c=c}
        "#;

    if let ExVal::Table(t) = simple(source_in) {
        assert_eq!(
            t.get("a"),
            Some(&ExVal::Integer(1))
        );
        assert_eq!(
            t.get("b"),
            Some(&ExVal::String("hello".to_string()))
        );
        assert_eq!(
            t.get("c"),
            Some(&ExVal::Bool(true))
        );
    } else {
        panic!("Expected table result");
    }
}

#[test]
fn multiple_returns_partial() {
    let source_in = r#"
        function get_values()
            return 10, 20, 30
        end
        
        local x, y = get_values()
        return {x=x, y=y}
        "#;

    if let ExVal::Table(t) = simple(source_in) {
        assert_eq!(
            t.get("x"),
            Some(&ExVal::Integer(10))
        );
        assert_eq!(
            t.get("y"),
            Some(&ExVal::Integer(20))
        );
    } else {
        panic!("Expected table result");
    }
}

#[test]
fn multiple_returns_extra() {
    let source_in = r#"
        function get_value()
            return 42
        end
        
        local a, b, c = get_value()
        return {a=a, b=b, c=c}
        "#;

    if let ExVal::Table(t) = simple(source_in) {
        assert_eq!(
            t.get("a"),
            Some(&ExVal::Integer(42))
        );
        assert_eq!(t.get("b"), Some(&ExVal::Nil));
        assert_eq!(t.get("c"), Some(&ExVal::Nil));
    } else {
        panic!("Expected table result");
    }
}
