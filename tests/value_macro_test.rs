#[cfg(test)]
mod tests {
    use silt_lua::Value;

    #[test]
    fn test_macro_conversions() {
        // Test primitive to Value conversions
        let v_u8: Value = 42u8.into();
        let v_u32: Value = 1000u32.into();
        let v_u64: Value = 999999u64.into();
        let v_i32: Value = <i32 as Into<Value>>::into(-123i32);
        let v_i64: Value = <i64 as Into<Value>>::into(-456i64);
        let v_f32: Value = 3.14f32.into();
        let v_f64: Value = 2.718f64.into();
        let v_bool: Value = true.into();

        // Test Value to primitive conversions
        let u32_val: u32 = v_u32.into();
        let u64_val: u64 = v_u64.into();
        let i32_val: i32 = v_i32.into();
        let i64_val: i64 = v_i64.into();
        let f32_val: f32 = v_f32.into();
        let f64_val: f64 = v_f64.into();

        assert_eq!(u32_val, 1000);
        assert_eq!(u64_val, 999999);
        assert_eq!(i32_val, -123);
        assert_eq!(i64_val, -456);
        assert_eq!(f32_val, 3.14);
        assert_eq!(f64_val, 2.718);

        // Test &Value to primitive conversions
        let u8_val_ref: u8 = (&v_u8).into();
        let bool_val_ref: bool = (&v_bool).into();

        assert_eq!(u8_val_ref, 42);
        assert_eq!(bool_val_ref, true);

        let u8_val: u8 = v_u8.into();
        let bool_val: bool = v_bool.into();
        assert_eq!(u8_val, 42);
        assert_eq!(bool_val, true);

        // A mid-range usize round-trips exactly.
        let v_usize: Value = 123456usize.into();
        let usize_val: usize = v_usize.into();
        assert_eq!(usize_val, 123456);

        // Values above i64::MAX saturate to i64::MAX (both directions) rather than
        // wrapping to u64::MAX / usize::MAX.
        let clamp = i64::MAX as u64;
        let v_big: Value = u64::MAX.into();
        assert_eq!(v_big, Value::Integer(i64::MAX));
        let big_back: u64 = v_big.into();
        assert_eq!(big_back, clamp);

        let v_big_usize: Value = usize::MAX.into();
        assert_eq!(v_big_usize, Value::Integer(i64::MAX));
        let big_usize_back: usize = v_big_usize.into();
        assert_eq!(big_usize_back, i64::MAX as usize);
    }
}
