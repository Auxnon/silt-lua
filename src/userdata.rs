pub trait UserData {
    fn get_type(&self) -> String;
    fn to_string(&self) -> String;
    fn get_meta_methods(&self) -> Option<MetaMethods> {
        None
    }
}

pub struct MetaMethods {
    // pub add: Option<fn(&UserData, &UserData) -> UserData>,
    // pub sub: Option<fn(&UserData, &UserData) -> UserData>,
    // pub mul: Option<fn(&UserData, &UserData) -> UserData>,
    // pub div: Option<fn(&UserData, &UserData) -> UserData>,
    // pub pow: Option<fn(&UserData, &UserData) -> UserData>,
    // pub concat: Option<fn(&UserData, &UserData) -> UserData>,
    // pub eq: Option<fn(&UserData, &UserData) -> UserData>,
    // pub lt: Option<fn(&UserData, &UserData) -> UserData>,
    // pub le: Option<fn(&UserData, &UserData) -> UserData>,
    // pub len: Option<fn(&UserData) -> UserData>,
    // pub tostring: Option<fn(&UserData) -> UserData>,
    // pub call: Option<fn(&UserData, Vec<UserData>) -> UserData>,
}

struct ExampleUserData {
    value: i64,
}

impl UserData for ExampleUserData {
    fn get_type(&self) -> String {
        "ExampleUserData".to_string()
    }
    fn to_string(&self) -> String {
        self.value.to_string()
    }
}
