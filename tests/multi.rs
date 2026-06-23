use silt_lua::{ExVal, simple, valeq};

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
        assert_eq!(t.get("a"), Some(&ExVal::Integer(2)));
        assert_eq!(t.get("b"), Some(&ExVal::String("hello".to_string())));
        assert_eq!(t.get("c"), Some(&ExVal::Bool(true)));
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
        assert_eq!(t.get("x"), Some(&ExVal::Integer(10)));
        assert_eq!(t.get("y"), Some(&ExVal::Integer(20)));
    } else {
        panic!("Expected table result");
    }
}

#[test]
#[ignore = "PLAN.md §2.13 — `local a,b,c = f()` with one return value OVERFLOWS THE STACK (SIGABRT)"]
fn multiple_returns_extra() {
    let source_in = r#"
        function get_value()
            return 42
        end
        
        local a, b, c = get_value()
        return {a=a, b=b, c=c}
        "#;

    if let ExVal::Table(t) = simple(source_in) {
        assert_eq!(t.get("a"), Some(&ExVal::Integer(42)));
        assert_eq!(t.get("b"), Some(&ExVal::Nil));
        assert_eq!(t.get("c"), Some(&ExVal::Nil));
    } else {
        panic!("Expected table result");
    }
}

#[test]
fn function_param() {
    valeq!(
        r#"
            function test(a,b)
                x=a
                y=b
                return x,y
            end
            u,v=test(3,8)
            u
            "#,
        ExVal::Integer(3)
    );

    valeq!(
        r#"
            function test(a,b)
                x=a
                y=b
                return x,y
            end
            u,v=test(3,8)
            v
            "#,
        ExVal::Integer(8)
    );
}
