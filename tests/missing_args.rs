use silt_lua::{simple, valeq, ExVal};

#[test]
fn missing_arg_is_nil() {
    valeq!(r#"
        D = "dflt"
        function f(x, y, w, h, col) return col or D end
        return f(1, 2, 3, 4)
    "#, ExVal::String("dflt".into()));
}

#[test]
fn missing_arg_is_nil_in_nested_call() {
    valeq!(r#"
        D = "dflt"
        function id(a, b, c, d, e) return e end
        function f(x, y, w, h, col) return id(x, y, w, h, col or D) end
        return f(1, 2, 3, 4)
    "#, ExVal::String("dflt".into()));
}

#[test]
fn missing_arg_is_nil_in_method_call() {
    valeq!(r#"
        D = "dflt"
        local o = {}
        function o:rect(a, b, c, d, e) return e end
        T = {}
        function T.panel(x, y, w, h, col)
            return o:rect(x, y, w, h, col or D)
        end
        return T.panel(1, 2, 3, 4)
    "#, ExVal::String("dflt".into()));
}
