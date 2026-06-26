//! Phase 1 of Luau-style static typing: a minimal type lattice plus the parsing
//! of type annotations. This phase only *tracks* types (records them on locals,
//! params, etc.) — it does not yet check or error. Everything here is gated
//! behind the `typing` cargo feature and has no effect on the runtime: types are
//! purely compile-time metadata and never reach the VM.
//!
//! The checking phases that build on this will use the [`assignable`] chokepoint
//! and a "type stack" that mirrors the compiler's operand stack.

/// A static type. Intentionally small for Phase 1; grow with `Union`, table
/// shapes, function signatures, and generics in later phases.
#[derive(Clone, Debug, PartialEq, Default)]
pub enum Type {
    /// Dynamic / unknown — assignable to and from anything (gradual typing).
    /// This is the default for everything not yet annotated or inferred.
    #[default]
    Any,
    Nil,
    Boolean,
    /// Lua unifies integer and float under one `number` type (as does `type()`).
    Number,
    String,
    /// Opaque table for now; a structural shape comes later.
    Table,
    /// Opaque callable for now; parameter/return signatures come later.
    Function,
    /// `T?` — value of `T` or `nil`. Parsing of the `?` suffix is deferred until
    /// the lexer emits a `?` token, but the variant exists so the lattice and
    /// `assignable` are ready for it.
    Optional(Box<Type>),
    /// A named type we don't resolve yet (user-defined alias, etc.). Treated as
    /// `Any` for assignability in Phase 1 so it never produces false errors.
    Named(String),
}

impl Type {
    /// Map a type-name identifier (or keyword spelling) to a `Type`.
    pub fn from_name(name: &str) -> Type {
        match name {
            "any" => Type::Any,
            "nil" => Type::Nil,
            "boolean" | "bool" => Type::Boolean,
            "number" => Type::Number,
            "string" => Type::String,
            "table" => Type::Table,
            "function" => Type::Function,
            other => Type::Named(other.to_string()),
        }
    }
}

/// Is a value of type `value` assignable to a slot of type `target`?
///
/// Phase 1 is deliberately lenient (gradual typing): `Any` and unresolved
/// `Named` types are compatible with everything, so the checker — once wired —
/// only ever complains when *both* sides are concretely known and incompatible.
pub fn assignable(target: &Type, value: &Type) -> bool {
    use Type::*;
    match (target, value) {
        // gradual: unknown on either side never errors
        (Any, _) | (_, Any) => true,
        (Named(_), _) | (_, Named(_)) => true,
        // nil fits any optional; an optional target also accepts its base type
        (Optional(_), Nil) => true,
        (Optional(t), v) => assignable(t, v),
        // assigning an optional into a non-optional: ok only if the inner matches
        // (the nil case would be unsound, but Phase 1 does not enforce it)
        (t, Optional(v)) => assignable(t, v),
        (a, b) => a == b,
    }
}
