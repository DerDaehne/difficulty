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
}

#[derive(Debug, Clone)]
pub struct TypeRef {
    pub name: String,
    pub pos: Pos,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub ty: TypeRef,
    pub name: String,
    pub pos: Pos,
}

#[derive(Debug, Clone)]
pub struct Func {
    pub ret: TypeRef,
    pub name: String,
    pub pos: Pos,
    pub params: Vec<Param>,
    pub body: Block,
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
    /// Placeholder after an error that was already reported.
    Missing,
}

#[derive(Debug, Clone)]
pub enum StrSeg {
    Lit(String),
    Expr(Expr),
}
