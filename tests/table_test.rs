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
fn test_array_length() {
    let source = r#"
        local t = {1, 2, 3, 4, 5}
        return #t
    "#;
    
    assert_eq!(simple(source), ExVal::Integer(5));
}

#[test]
fn test_array_indexed_access() {
    let source = r#"
        local t = {10, 20, 30, 40}
        return t[3]
    "#;
    
    assert_eq!(simple(source), ExVal::Integer(30));
}

#[test]
fn test_array_push() {
    let source = r#"
        local t = {}
        t[1] = 1
        t[2] = 2
        t[3] = 3
        return #t
    "#;
    
    assert_eq!(simple(source), ExVal::Integer(3));
}

#[test]
fn test_mixed_array_hash() {
    let source = r#"
        local t = {1, 2, 3}
        t.name = "test"
        t.value = 42
        return {len=#t, name=t.name, value=t.value, first=t[1]}
    "#;
    
    if let ExVal::Table(t) = simple(source) {
        assert_eq!(t.get("len"), Some(&ExVal::Integer(3)));
        assert_eq!(t.get("name"), Some(&ExVal::String("test".to_string())));
        assert_eq!(t.get("value"), Some(&ExVal::Integer(42)));
        assert_eq!(t.get("first"), Some(&ExVal::Integer(1)));
    } else {
        panic!("Expected table result");
    }
}

#[test]
fn test_array_sequential_assignment() {
    let source = r#"
        local t = {}
        t[1] = "a"
        t[2] = "b"
        t[3] = "c"
        return {len=#t, v1=t[1], v2=t[2], v3=t[3]}
    "#;
    
    if let ExVal::Table(t) = simple(source) {
        assert_eq!(t.get("len"), Some(&ExVal::Integer(3)));
        assert_eq!(t.get("v1"), Some(&ExVal::String("a".to_string())));
        assert_eq!(t.get("v2"), Some(&ExVal::String("b".to_string())));
        assert_eq!(t.get("v3"), Some(&ExVal::String("c".to_string())));
    } else {
        panic!("Expected table result");
    }
}

#[test]
fn test_hash_keys() {
    let source = r#"
        local t = {}
        t["name"] = "John"
        t["age"] = 30
        t["active"] = true
        return {name=t.name, age=t.age, active=t.active}
    "#;
    
    if let ExVal::Table(t) = simple(source) {
        assert_eq!(t.get("name"), Some(&ExVal::String("John".to_string())));
        assert_eq!(t.get("age"), Some(&ExVal::Integer(30)));
        assert_eq!(t.get("active"), Some(&ExVal::Bool(true)));
    } else {
        panic!("Expected table result");
    }
}

#[test]
fn test_negative_index() {
    let source = r#"
        local t = {}
        t[-1] = "negative"
        t[1] = "positive"
        return {neg=t[-1], pos=t[1], len=#t}
    "#;
    
    if let ExVal::Table(t) = simple(source) {
        assert_eq!(t.get("neg"), Some(&ExVal::String("negative".to_string())));
        assert_eq!(t.get("pos"), Some(&ExVal::String("positive".to_string())));
        // Negative indices don't count towards array length
        assert_eq!(t.get("len"), Some(&ExVal::Integer(1)));
    } else {
        panic!("Expected table result");
    }
}

#[test]
fn test_large_index() {
    let source = r#"
        local t = {}
        t[1000000] = "large"
        t[1] = "small"
        return {large=t[1000000], small=t[1], len=#t}
    "#;
    
    if let ExVal::Table(t) = simple(source) {
        assert_eq!(t.get("large"), Some(&ExVal::String("large".to_string())));
        assert_eq!(t.get("small"), Some(&ExVal::String("small".to_string())));
        // Large indices don't extend the array
        assert_eq!(t.get("len"), Some(&ExVal::Integer(1)));
    } else {
        panic!("Expected table result");
    }
}

#[test]
fn test_array_with_holes() {
    let source = r#"
        local t = {}
        t[1] = "a"
        t[3] = "c"
        t[5] = "e"
        return {len=#t, v1=t[1], v2=t[2], v3=t[3]}
    "#;
    
    if let ExVal::Table(t) = simple(source) {
        // Lua's length operator behavior with holes can be tricky
        // Our implementation should handle this gracefully
        assert_eq!(t.get("v1"), Some(&ExVal::String("a".to_string())));
        assert_eq!(t.get("v2"), Some(&ExVal::Nil));
        assert_eq!(t.get("v3"), Some(&ExVal::String("c".to_string())));
    } else {
        panic!("Expected table result");
    }
}

#[test]
fn test_cyclic_reference_detection() {
    let source = r#"
        local t1 = {value = 1}
        local t2 = {value = 2}
        t1.next = t2
        t2.prev = t1
        -- This creates a cycle: t1 -> t2 -> t1
        return {v1=t1.value, v2=t2.value}
    "#;
    
    // This test verifies that cyclic references don't cause crashes
    // The cycle detection should handle this gracefully
    if let ExVal::Table(t) = simple(source) {
        assert_eq!(t.get("v1"), Some(&ExVal::Integer(1)));
        assert_eq!(t.get("v2"), Some(&ExVal::Integer(2)));
    } else {
        panic!("Expected table result");
    }
}

#[test]
fn test_table_length_operator() {
    let source = r#"
        local t = {10, 20, 30}
        local len = #t
        t[4] = 40
        local len2 = #t
        return {len1=len, len2=len2}
    "#;
    
    if let ExVal::Table(t) = simple(source) {
        assert_eq!(t.get("len1"), Some(&ExVal::Integer(3)));
        assert_eq!(t.get("len2"), Some(&ExVal::Integer(4)));
    } else {
        panic!("Expected table result");
    }
}

#[test]
fn test_empty_table() {
    let source = r#"
        local t = {}
        return #t
    "#;
    
    assert_eq!(simple(source), ExVal::Integer(0));
}

#[test]
fn test_table_with_only_hash() {
    let source = r#"
        local t = {name="test", value=42}
        return #t
    "#;
    
    // Hash-only tables should have length 0
    assert_eq!(simple(source), ExVal::Integer(0));
}

#[cfg(feature = "serde")]
#[test]
#[ignore] // JSON serialization requires ExVal keys to implement special serde traits
fn test_serialization_with_string_keys() {
    use serde_json;
    
    // NOTE: This test is ignored because serde_json requires HashMap keys to implement
    // serde::Serialize in a specific way. ExVal would need custom Serialize implementation
    // to support this. The Serialize/Deserialize derives are provided for other serialization
    // formats that may support arbitrary key types (e.g., bincode, messagepack).
    
    let source = r#"
        return {name="test", value="42", active="true"}
    "#;
    
    if let ExVal::Table(t) = simple(source) {
        // This will fail because serde_json can't serialize ExVal keys
        let json = serde_json::to_string(&t);
        // Expected to fail with "key must be a string" error
        assert!(json.is_err());
    } else {
        panic!("Expected table result");
    }
}

#[cfg(feature = "serde")]
#[test]
#[ignore] // JSON serialization doesn't support non-string keys in HashMaps (array indices)
fn test_serialization_with_mixed_keys() {
    use serde_json;
    
    // This test documents the limitation with JSON serialization
    let source = r#"
        return {1, 2, 3, name="test"}  -- Mixed integer and string keys
    "#;
    
    if let ExVal::Table(t) = simple(source) {
        // This will fail because JSON can't serialize integer keys
        let json = serde_json::to_string(&t);
        assert!(json.is_err(), "JSON serialization should fail with non-string keys");
    } else {
        panic!("Expected table result");
    }
}
