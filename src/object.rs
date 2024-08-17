struct Lob<'gc, T: 'gc> {}
impl Lob {
    pub fn new(value: Value) -> Self {
        Lobj { value, next: None }
    }
}
type LobPtr<'gc, T> = Gc<'gc, RefLock<Lob<'gc, T>>>;

fn new_node<'gc, T: Collect>(mc: &Mutation<'gc>, value: T) -> LobPtr<'gc, T> {
    Gc::new(
        mc,
        RefLock::new(Node {
            prev: None,
            next: None,
            value,
        }),
    )
}
