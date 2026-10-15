use silt_lua::{simple, valeq, ExVal};

// SILT-BUGS.md #5: a table constructor holding a local, assigned through an index
// (`t[#t + 1] = { v, 2 }`), corrupted the chunk.
#[test]
fn index_assign_ctor_with_local() {
    valeq!(
        r#"
        function f()
            local vs = { 1 }
            local x = { 1, 2 }
            x[#x + 1] = { vs, 2 }
            return #x
        end
        return f()
        "#,
        ExVal::Integer(3)
    );
}

// SILT-BUGS.md #15: the same with a field target (`e.size = { k, 1, k }`) failed with
// "Invalid chunk due compilation corruption".
#[test]
fn field_assign_ctor_with_locals() {
    valeq!(
        r#"
        function f()
            local k = 3
            local e = {}
            e.size = { k, 1, k }
            return e.size[1] + e.size[3]
        end
        return f()
        "#,
        ExVal::Integer(6)
    );
}

#[test]
fn field_assign_ctor_with_field_products() {
    valeq!(
        r#"
        function f()
            local r = { w = 2 }
            local k = 3
            local e = {}
            e.size = { r.w * k, 1, k }
            return e.size[1]
        end
        return f()
        "#,
        ExVal::Integer(6)
    );
}

// The shape Chor's HUD hit: another field assignment first, products in the constructor.
#[test]
fn field_assign_ctor_after_field_assign() {
    valeq!(
        r#"
        function f()
            local r = { w = 2 }
            local k = 3
            local grow = 2
            local e = {}
            local d = 1
            e.z = (r.w + d) * k
            e.size = { r.w * k * grow, 1, r.w * k * grow }
            return e.size[1] + e.z
        end
        return f()
        "#,
        ExVal::Integer(21)
    );
}

#[test]
fn nested_ctor_and_call_with_locals_into_index() {
    valeq!(
        r#"
        function id(x) return x end
        local a = 4
        local b = 5
        local t = {}
        t["k"] = { a, { b, a }, id(b), x = a }
        return t.k[1] + t.k[2][1] + t.k[3] + t.k.x
        "#,
        ExVal::Integer(18)
    );
}

// Multi-assignment and plain-local assignment are unaffected.
#[test]
fn multi_assign_still_works() {
    valeq!(
        r#"
        local a, b = 1, 2
        local t = {}
        a, b = b, a
        t.x = a
        local c = { a, b }
        return t.x * 10 + c[2]
        "#,
        ExVal::Integer(21)
    );
}
