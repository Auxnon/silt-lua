use silt_lua::{simple, valeq};

// #[allow(unused_macros)]
// macro_rules! valeq {
//     ($source:literal, $val:expr) => {
//         assert_eq!(
//             simple($source),
//             $val.into(),
//             "output does not match expected value"
//         );
//     };
// }

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
