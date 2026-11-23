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
}

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

#[derive(Debug, Clone)]
pub enum Pattern {
    /// An integer literal, already negated for `-5`.
    Int(i128, Pos),
    Str(String, Pos),
    /// A `const` name, or an unqualified variant (an error the checker reports).
    Name(String, Pos),
    /// `E.V`, or `E.V(T a, _)` with bindings (`None` is `_`).
    Variant {
        ty: String,
        name: String,
        binds: Option<Vec<Option<Param>>>,
        pos: Pos,
    },
}

/// A branch of `if` as a value: statements, then the value.
#[derive(Debug, Clone)]
pub struct ValueBlock {
    pub stmts: Vec<Stmt>,
    pub value: Expr,
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
    Bool(bool),
    Str(Vec<StrSeg>),
    Ident(String),
    Unary(&'static str, Box<Expr>),
    /// The last field is the position of the operator.
    Binary(&'static str, Box<Expr>, Box<Expr>, Pos),
    Call(Box<Expr>, Vec<Expr>),
    Index(Box<Expr>, Box<Expr>),
    Field(Box<Expr>, String),
    Paren(Box<Expr>),
    SelfRef,
    /// `T{f: v, …}`; the position of each name is kept for diagnostics.
    StructLit(String, Vec<FieldInit>),
    /// `if c { … } else if … else { … }` as a value.
    IfExpr(Vec<(Expr, ValueBlock)>, Box<ValueBlock>),
    /// `mut place` as an argument (R6).
    Mut(Box<Expr>),
    /// Placeholder after an error that was already reported.
    Missing,
}

#[derive(Debug, Clone)]
pub enum StrSeg {
    Lit(String),
    Expr(Expr),
}
