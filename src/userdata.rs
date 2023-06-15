trait UserData {
    fn get_type(&self) -> String;
}

struct ExampleUserData {
    value: i64,
}

impl UserData for ExampleUserData {
    fn get_type(&self) -> String {
        "ExampleUserData".to_string()
    }
}
