//! Syntax tree of a module. Every node keeps its start position; expressions also
//! keep the byte offset of their end so diagnostics can quote the source.

use crate::diag::Pos;

#[derive(Debug, Clone)]
pub struct Module {
    pub decls: Vec<Decl>,
}

#[derive(Debug, Clone)]
pub enum Decl {
    Const(VarDecl),
    Func(Func),
    Struct(StructDecl),
    Enum(EnumDecl),
    Test(TestDecl),
    Use(UseDecl),
}

/// `use std.io` (R16).
#[derive(Debug, Clone)]
pub struct UseDecl {
    /// The module after `std.`.
    pub module: String,
    /// Position of that name.
    pub name_pos: Pos,
    /// Position of the word `use`.
    pub pos: Pos,
}

/// `test "name" [panics "text"] { … }` (R18).
#[derive(Debug, Clone)]
pub struct TestDecl {
    pub name: String,
    /// Position of the name string.
    pub name_pos: Pos,
    pub panics: Option<(String, Pos)>,
    pub body: Block,
    /// Position of the word `test`.
    pub pos: Pos,
}

/// A written type; `name` is its text without spaces: `int`, `?int`, `int!AgeErr`, `void!E`.
#[derive(Debug, Clone)]
pub struct TypeRef {
    pub name: String,
    pub pos: Pos,
}

/// A parameter, a loop variable, a field of an enum variant or a case binding.
#[derive(Debug, Clone)]
pub struct Param {
    pub ty: TypeRef,
    pub name: String,
    pub pos: Pos,
    /// Position of `mut` before the type (parameters only).
    pub mut_pos: Option<Pos>,
}

/// A function, or a method `R T.name(…)` when `recv` names the type.
#[derive(Debug, Clone)]
pub struct Func {
    pub ret: TypeRef,
    pub recv: Option<TypeRef>,
    pub name: String,
    pub pos: Pos,
    /// Position of `mut self` (methods only).
    pub mut_self: Option<Pos>,
    pub params: Vec<Param>,
    pub body: Block,
}

#[derive(Debug, Clone)]
pub struct StructDecl {
    pub name: String,
    pub pos: Pos,
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone)]
pub struct Field {
    pub ty: TypeRef,
    pub name: String,
    pub pos: Pos,
    pub default: Option<Expr>,
}

#[derive(Debug, Clone)]
pub struct EnumDecl {
    pub name: String,
    pub pos: Pos,
    pub variants: Vec<Variant>,
}

#[derive(Debug, Clone)]
pub struct Variant {
    pub name: String,
    pub pos: Pos,
    pub fields: Vec<Param>,
}

#[derive(Debug, Clone)]
pub struct Case {
    pub pats: Vec<Pattern>,
    pub body: Block,
    pub pos: Pos,
}

/// The `or` part of `try`: the position of `or`, the cause binding `err(E e):` if
/// written, and the value (R12).
#[derive(Debug, Clone)]
pub struct TryOr {
    pub pos: Pos,
    pub bind: Option<Param>,
    pub alt: Box<Expr>,
}

#[derive(Debug, Clone)]
pub enum Pattern {
    /// An integer literal, already negated for `-5`.
    Int(i128, Pos),
    Str(String, Pos),
    Rune(u32, Pos),
    /// A `const` name, or an unqualified variant (an error the checker reports).
    Name(String, Pos),
    /// `E.V`, or `E.V(T a, _)` with bindings (`None` is `_`).
    Variant {
        ty: String,
        name: String,
        binds: Option<Vec<Option<Param>>>,
        pos: Pos,
    },
    /// `ok`, `err`, `some`, `none` with an optional binding: the outer `None` means no
    /// parentheses, `Some(None)` means `(_)`.
    Result {
        kind: &'static str,
        bind: Option<Option<Param>>,
        pos: Pos,
    },
}

/// A branch of `if` as a value: statements, then the value. A branch that ends in
/// `return`, `break` or `continue` never arrives and has no value (R15).
#[derive(Debug, Clone)]
pub struct ValueBlock {
    pub stmts: Vec<Stmt>,
    pub value: Option<Expr>,
    pub pos: Pos,
}

#[derive(Debug, Clone)]
pub struct FieldInit {
    pub name: String,
    pub pos: Pos,
    pub value: Expr,
}

/// `[const] T name = value`. A declaration written in foreign style (`var x = 5`)
/// keeps the type the parser could derive, or `?` when it could not.
#[derive(Debug, Clone)]
pub struct VarDecl {
    pub is_const: bool,
    pub ty: TypeRef,
    pub name: String,
    pub pos: Pos,
    pub value: Expr,
}

#[derive(Debug, Clone)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub pos: Pos,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Var(VarDecl),
    Assign {
        place: Expr,
        op: &'static str,
        value: Expr,
        pos: Pos,
    },
    If {
        branches: Vec<(Expr, Block)>,
        otherwise: Option<Block>,
        pos: Pos,
    },
    While {
        cond: Expr,
        body: Block,
        pos: Pos,
    },
    /// `for T x in e` has one variable; `for int i, T x in e` has two (index first).
    For {
        vars: Vec<Param>,
        iter: Expr,
        body: Block,
        pos: Pos,
    },
    Switch {
        subject: Expr,
        cases: Vec<Case>,
        default: Option<(Block, Pos)>,
        pos: Pos,
    },
    Break(Pos),
    Continue(Pos),
    Return(Option<Expr>, Pos),
    Call(Expr),
    Discard(Expr, Pos),
}

#[derive(Debug, Clone)]
pub struct Expr {
    pub kind: ExprKind,
    pub pos: Pos,
    pub end: u32,
}

#[derive(Debug, Clone)]
pub enum ExprKind {
    Int(u128),
    Float(String),
    /// A rune literal: one code point.
    Rune(u32),
    Bool(bool),
    Str(Vec<StrSeg>),
    Ident(String),
    Unary(&'static str, Box<Expr>),
    /// The last field is the position of the operator.
    Binary(&'static str, Box<Expr>, Box<Expr>, Pos),
    Call(Box<Expr>, Vec<Expr>),
    Index(Box<Expr>, Box<Expr>, Pos),
    Field(Box<Expr>, String),
    Paren(Box<Expr>),
    SelfRef,
    /// `T{f: v, …}`; the position of each name is kept for diagnostics.
    StructLit(String, Vec<FieldInit>),
    /// `if c { … } else if … else { … }` as a value.
    IfExpr(Vec<(Expr, ValueBlock)>, Box<ValueBlock>),
    /// `mut place` as an argument (R6).
    Mut(Box<Expr>),
    /// `none`
    NoneVal,
    /// `some(x)`
    SomeVal(Box<Expr>),
    /// `err(e)`
    ErrVal(Box<Expr>),
    /// `ok(x)` or `ok`: only a pattern (R12), kept so the checker can report it.
    OkVal(Option<Box<Expr>>),
    /// `try x`, or `try x or e` / `try x or err(E e): e2`.
    Try(Box<Expr>, Option<TryOr>),
    /// `[a, b]`
    ListLit(Vec<Expr>),
    /// `{k: v, …}`
    MapLit(Vec<(Expr, Expr)>),
    /// Placeholder after an error that was already reported.
    Missing,
}

#[derive(Debug, Clone)]
pub enum StrSeg {
    Lit(String),
    Expr(Expr),
    /// `{x:g}` or `{x:.2f}`.
    Fmt(Expr, crate::lexer::Fmt),
}
