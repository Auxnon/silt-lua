//! Luau-style gradual static types. Compile-time only: no `Type` ever reaches
//! the VM and bytecode is identical with or without annotations.
//!
//! The lattice is deliberately gradual: `Any` (and an alias name that never
//! resolved) is compatible with everything in both directions, so the checker
//! only ever complains when *both* sides are concretely known and incompatible.
//! `assignable` is the single compatibility chokepoint.

use std::fmt;

/// A static type.
#[derive(Clone, Debug, PartialEq, Default)]
pub enum Type {
    /// Dynamic / unknown — assignable to and from anything (gradual typing).
    /// This is the default for everything not annotated or inferred.
    #[default]
    Any,
    Nil,
    Boolean,
    /// Lua unifies integer and float under one `number` type (as does `type()`).
    Number,
    String,
    /// A singleton string type: `"up"` (Luau). String constants have this type,
    /// so `"up" | "down"` rejects `"left"`; it fits any `string` slot.
    StringLit(String),
    /// A singleton boolean type: `true` / `false`.
    BoolLit(bool),
    /// A table; `Some(shape)` when its fields are known (`{x: number}`, `{T}`).
    Table(Option<Box<TableShape>>),
    /// A callable; `Some(sig)` when its parameters/returns are known.
    Function(Option<Box<FnSig>>),
    /// A function that can leave the VM as a snippet (`snippet`,
    /// `snippet(number, number) -> number`): it captures and references values
    /// only, never functions or other references, so an isolated VM can run it.
    /// A snippet is a function in every other sense — it fits any `function`
    /// slot — but only a function literal the checker verified fits a `snippet`
    /// slot. Only relevant to a host that extracts snippets (`snippets` feature).
    Snippet(Option<Box<FnSig>>),
    /// `T?` — a value of `T` or `nil`.
    Optional(Box<Type>),
    /// `A | B`.
    Union(Vec<Type>),
    /// A type alias by name, resolved against the compile's alias table at check
    /// time. Unknown names stay `Named` and behave as `Any`.
    Named(String),
}

/// A function signature: `(params) -> returns`.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct FnSig {
    pub params: Vec<Type>,
    /// Accepts extra arguments (`...`).
    pub variadic: bool,
    /// `None` when the return types are not declared (results are `Any`).
    pub returns: Option<Vec<Type>>,
    /// Whether the function this signature describes satisfies the snippet
    /// rules. Set by the checker on every function literal; `Unknown` for a
    /// signature that came from an annotation or a declaration.
    pub snippet: Snippetness,
}

/// Can a function be extracted as a snippet? Decided per function literal at
/// compile time from what its body captures and references.
#[derive(Clone, Debug, PartialEq, Default)]
pub enum Snippetness {
    /// Not a literal we analysed (an annotation, a declaration, a value of
    /// unknown origin): gradual — accepted where a snippet is expected.
    #[default]
    Unknown,
    /// Every capture and global reference is a value (or resolvable in the
    /// snippet VM).
    Yes,
    /// Provably not a snippet; the string says why (names the offending
    /// capture or global).
    No(String),
}

/// A structural table type.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct TableShape {
    /// Named fields, in declaration order.
    pub fields: Vec<(String, Type)>,
    /// `{T}` — element type of the array part.
    pub array: Option<Type>,
    /// `{[K]: V}` — an index signature.
    pub index: Option<(Type, Type)>,
}

impl TableShape {
    pub fn field(&self, name: &str) -> Option<&Type> {
        self.fields.iter().find(|(n, _)| n == name).map(|(_, t)| t)
    }
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
            "table" => Type::Table(None),
            "function" => Type::Function(None),
            "snippet" => Type::Snippet(None),
            other => Type::Named(other.to_string()),
        }
    }

    /// The type `type(v)` would report for a value of this type, if it is one
    /// of the primitive names.
    pub fn from_type_name(name: &str) -> Option<Type> {
        match name {
            "nil" => Some(Type::Nil),
            "boolean" => Some(Type::Boolean),
            "number" => Some(Type::Number),
            "string" => Some(Type::String),
            "table" => Some(Type::Table(None)),
            "function" => Some(Type::Function(None)),
            _ => None,
        }
    }

    /// `Any` or an unresolved name: nothing is known.
    pub fn is_unknown(&self) -> bool {
        matches!(self, Type::Any | Type::Named(_))
    }

    pub fn is_nil(&self) -> bool {
        matches!(self, Type::Nil)
    }

    /// Singleton types widened to their base type (`"up"` → `string`), for
    /// operator checks and for inferring a variable's type from its initializer.
    pub fn widen(&self) -> Type {
        match self {
            Type::StringLit(_) => Type::String,
            Type::BoolLit(_) => Type::Boolean,
            Type::Optional(t) => Type::Optional(Box::new(t.widen())),
            Type::Union(parts) => Type::union(parts.iter().map(|t| t.widen()).collect()),
            t => t.clone(),
        }
    }

    /// Does this type name a singleton (`"up"`, `true`) at its top level?
    pub fn has_literal(&self) -> bool {
        match self {
            Type::StringLit(_) | Type::BoolLit(_) => true,
            Type::Optional(t) => t.has_literal(),
            Type::Union(parts) => parts.iter().any(|t| t.has_literal()),
            _ => false,
        }
    }

    /// The name `type(v)` reports for every value of this type, if fixed.
    pub fn type_name(&self) -> Option<&'static str> {
        Some(match self {
            Type::Nil => "nil",
            Type::Boolean | Type::BoolLit(_) => "boolean",
            Type::Number => "number",
            Type::String | Type::StringLit(_) => "string",
            Type::Table(_) => "table",
            Type::Function(_) | Type::Snippet(_) => "function",
            _ => return None,
        })
    }

    /// Split a union for a `type(v) == name` test into the members that pass and
    /// those that don't; `None` unless both sides are non-empty.
    pub fn split_type_name(&self, name: &str) -> Option<(Type, Type)> {
        let parts: Vec<Type> = match self {
            Type::Union(p) => p.clone(),
            Type::Optional(t) => vec![(**t).clone(), Type::Nil],
            _ => return None,
        };
        let (yes, no): (Vec<Type>, Vec<Type>) =
            parts.into_iter().partition(|p| p.type_name() == Some(name));
        if yes.is_empty() || no.is_empty() {
            return None;
        }
        Some((Type::union(yes), Type::union(no)))
    }

    /// `A | B` with duplicates and nested unions folded; `T | nil` becomes `T?`.
    pub fn union(parts: Vec<Type>) -> Type {
        let mut flat: Vec<Type> = Vec::new();
        for p in parts {
            match p {
                Type::Union(inner) => {
                    for t in inner {
                        if !flat.contains(&t) {
                            flat.push(t);
                        }
                    }
                }
                Type::Optional(t) => {
                    if !flat.contains(&t) {
                        flat.push(*t);
                    }
                    if !flat.contains(&Type::Nil) {
                        flat.push(Type::Nil);
                    }
                }
                t => {
                    if !flat.contains(&t) {
                        flat.push(t);
                    }
                }
            }
        }
        if flat.iter().any(|t| t.is_unknown()) {
            return Type::Any;
        }
        if flat.len() == 2 && flat.contains(&Type::Nil) {
            let t = flat.into_iter().find(|t| !t.is_nil()).unwrap();
            return Type::Optional(Box::new(t));
        }
        match flat.len() {
            0 => Type::Any,
            1 => flat.pop().unwrap(),
            _ => Type::Union(flat),
        }
    }

    /// The type after a truthiness test succeeded: `nil` can no longer be the
    /// value (`T?` → `T`; `A | nil` → `A`).
    pub fn truthy(&self) -> Type {
        match self {
            Type::Optional(t) => (**t).clone(),
            Type::Union(parts) => {
                Type::union(parts.iter().filter(|t| !t.is_nil()).cloned().collect())
            }
            t => t.clone(),
        }
    }

    /// Does this type admit `nil`?
    pub fn allows_nil(&self) -> bool {
        match self {
            Type::Any | Type::Named(_) | Type::Nil | Type::Optional(_) => true,
            Type::Union(parts) => parts.iter().any(|t| t.allows_nil()),
            _ => false,
        }
    }
}

/// Is a value of type `value` assignable to a slot of type `target`?
///
/// Gradual: `Any` and unresolved `Named` types are compatible with everything,
/// so callers resolve aliases first and the checker only complains when both
/// sides are concretely known and incompatible.
pub fn assignable(target: &Type, value: &Type) -> bool {
    use Type::*;
    match (target, value) {
        (Any, _) | (_, Any) => true,
        (Named(_), _) | (_, Named(_)) => true,
        // a union value fits when every member fits
        (t, Union(vs)) => vs.iter().all(|v| assignable(t, v)),
        // nil fits any optional; an optional target also accepts its base type
        (Optional(_), Nil) => true,
        (Optional(t), Optional(v)) => assignable(t, v),
        (Optional(t), v) => assignable(t, v),
        // `T?` into a non-optional slot only if that slot takes nil too
        (t, Optional(v)) => assignable(t, &Nil) && assignable(t, v),
        // a union target accepts any member
        (Union(ts), v) => ts.iter().any(|t| assignable(t, v)),
        // tables: unknown shape on either side is fine; otherwise width subtyping
        (Table(None), Table(_)) | (Table(_), Table(None)) => true,
        (Table(Some(a)), Table(Some(b))) => shape_assignable(a, b),
        // functions: unknown signature on either side is fine
        (Function(None), Function(_)) | (Function(_), Function(None)) => true,
        (Function(Some(a)), Function(Some(b))) => sig_assignable(a, b),
        // a snippet is a function
        (Function(None), Snippet(_)) | (Function(_), Snippet(None)) => true,
        (Function(Some(a)), Snippet(Some(b))) => sig_assignable(a, b),
        // a function is a snippet only if the checker did not prove otherwise
        (Snippet(_), Function(Some(v))) if matches!(v.snippet, Snippetness::No(_)) => false,
        (Snippet(None), Function(_) | Snippet(_)) | (Snippet(_), Function(None) | Snippet(None)) => {
            true
        }
        (Snippet(Some(a)), Function(Some(b)) | Snippet(Some(b))) => sig_assignable(a, b),
        // a singleton fits its base type (not the reverse)
        (String, StringLit(_)) | (Boolean, BoolLit(_)) => true,
        (a, b) => a == b,
    }
}

/// Every field the target shape names must exist in the value shape with a
/// compatible type; extra fields in the value are fine (width subtyping).
fn shape_assignable(target: &TableShape, value: &TableShape) -> bool {
    for (name, t) in &target.fields {
        match value.field(name) {
            Some(v) => {
                if !assignable(t, v) {
                    return false;
                }
            }
            // a missing field is nil — fine only if the target field is optional
            None => {
                if !t.allows_nil() {
                    return false;
                }
            }
        }
    }
    if let (Some(t), Some(v)) = (&target.array, &value.array) {
        if !assignable(t, v) {
            return false;
        }
    }
    true
}

/// Parameters are checked contravariantly, returns covariantly; arity must
/// match unless the target is variadic.
fn sig_assignable(target: &FnSig, value: &FnSig) -> bool {
    if value.params.len() > target.params.len() && !target.variadic {
        return false;
    }
    for (t, v) in target.params.iter().zip(value.params.iter()) {
        if !assignable(v, t) {
            return false;
        }
    }
    // the value must supply every declared return, each of a compatible type
    match (&target.returns, &value.returns) {
        (Some(t), Some(v)) => {
            t.len() <= v.len() && t.iter().zip(v.iter()).all(|(a, b)| assignable(a, b))
        }
        _ => true,
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Any => write!(f, "any"),
            Type::Nil => write!(f, "nil"),
            Type::Boolean => write!(f, "boolean"),
            Type::Number => write!(f, "number"),
            Type::String => write!(f, "string"),
            Type::StringLit(s) => write!(f, "{:?}", s),
            Type::BoolLit(b) => write!(f, "{}", b),
            Type::Table(None) => write!(f, "table"),
            Type::Table(Some(s)) => write!(f, "{}", s),
            Type::Function(None) => write!(f, "function"),
            Type::Function(Some(s)) => write!(f, "{}", s),
            Type::Snippet(None) => write!(f, "snippet"),
            Type::Snippet(Some(s)) => write!(f, "snippet{}", s),
            Type::Optional(t) => match **t {
                Type::Union(_) | Type::Function(Some(_)) | Type::Snippet(Some(_)) => {
                    write!(f, "({})?", t)
                }
                _ => write!(f, "{}?", t),
            },
            Type::Union(parts) => {
                for (i, p) in parts.iter().enumerate() {
                    if i > 0 {
                        write!(f, " | ")?;
                    }
                    write!(f, "{}", p)?;
                }
                Ok(())
            }
            Type::Named(n) => write!(f, "{}", n),
        }
    }
}

impl fmt::Display for FnSig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "(")?;
        for (i, p) in self.params.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}", p)?;
        }
        if self.variadic {
            if !self.params.is_empty() {
                write!(f, ", ")?;
            }
            write!(f, "...")?;
        }
        write!(f, ") -> ")?;
        match &self.returns {
            None => write!(f, "any"),
            Some(r) if r.len() == 1 => write!(f, "{}", r[0]),
            Some(r) => {
                write!(f, "(")?;
                for (i, t) in r.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", t)?;
                }
                write!(f, ")")
            }
        }
    }
}

impl fmt::Display for TableShape {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{{")?;
        let mut first = true;
        if let Some((k, v)) = &self.index {
            write!(f, "[{}]: {}", k, v)?;
            first = false;
        }
        if let Some(a) = &self.array {
            if !first {
                write!(f, ", ")?;
            }
            write!(f, "{}", a)?;
            first = false;
        }
        for (n, t) in &self.fields {
            if !first {
                write!(f, ", ")?;
            }
            write!(f, "{}: {}", n, t)?;
            first = false;
        }
        write!(f, "}}")
    }
}
