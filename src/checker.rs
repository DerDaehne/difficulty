//! Names and types (spec R1–R11, R13–R15, R28, R29). Runs only on a module without
//! lexical or syntax errors, so every diagnostic here has its own cause.
//!
//! Two passes: the first collects what each function does directly (prints,
//! changes a parameter or `self`, calls whom with what); a fixpoint derives which
//! functions change their `mut` parameters and `self` and which have an effect
//! (R6, R7, R13). The second pass reports, using those derived facts.

use std::collections::{HashMap, HashSet};

use crate::ast::*;
use crate::diag::{Diag, Pos};
use crate::lexer::Conv;
use crate::live::{self, CallChanges};

mod lit;
mod p2c;

/// The sized number types of part 2c (R8); `int` is `Ty::Int`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Num {
    I8,
    I16,
    I32,
    U8,
    U16,
    U32,
    U64,
    F64,
}

impl Num {
    pub const ALL: [Num; 8] = [
        Num::I8,
        Num::I16,
        Num::I32,
        Num::U8,
        Num::U16,
        Num::U32,
        Num::U64,
        Num::F64,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Num::I8 => "i8",
            Num::I16 => "i16",
            Num::I32 => "i32",
            Num::U8 => "u8",
            Num::U16 => "u16",
            Num::U32 => "u32",
            Num::U64 => "u64",
            Num::F64 => "f64",
        }
    }

    pub fn bits(self) -> u32 {
        match self {
            Num::I8 | Num::U8 => 8,
            Num::I16 | Num::U16 => 16,
            Num::I32 | Num::U32 => 32,
            Num::U64 | Num::F64 => 64,
        }
    }

    pub fn signed(self) -> bool {
        matches!(self, Num::I8 | Num::I16 | Num::I32 | Num::F64)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Ty {
    Int,
    Bool,
    Str,
    Void,
    /// `i8 … u64` and `f64` (part 2c).
    Num(Num),
    Rune,
    /// An integer literal expression that has no type yet: it takes the type of its
    /// place (R8); without one it is `int`.
    IntLit,
    /// A float literal expression: `f64`.
    FloatLit,
    /// A `struct` or `enum` of the module, by index in the type table.
    Named(u32),
    /// `?T`: index of `T` in `Info::opts`.
    Opt(u32),
    /// `T!E`: index of `(T, E)` in `Info::ress`.
    Res(u32),
    /// `[]T`: index of `T` in `Info::lists`.
    List(u32),
    /// `[N]T`: index of `(T, N)` in `Info::arrs`.
    Arr(u32),
    /// `map[K]V`: index of `(K, V)` in `Info::maps`.
    Map(u32),
    /// `R(T1, mut T2)`: index in `Info::ftys`. Only the type of a parameter, a literal or a block.
    Func(u32),
    /// Already reported; silences everything that depends on it.
    Err,
}

impl Ty {
    /// `int`, `i8 … u64` (a literal counts as `int`).
    fn is_int(self) -> bool {
        matches!(self, Ty::Int | Ty::IntLit) || matches!(self, Ty::Num(n) if n != Num::F64)
    }

    fn is_float(self) -> bool {
        matches!(self, Ty::Num(Num::F64) | Ty::FloatLit)
    }

    /// A number or a literal: the types R8 lets a literal take.
    pub(crate) fn is_number(self) -> bool {
        self.is_int() || self.is_float()
    }

    fn is_lit(self) -> bool {
        matches!(self, Ty::IntLit | Ty::FloatLit)
    }

    /// Range of an integer type.
    fn range(self) -> Option<(i128, i128)> {
        match self {
            Ty::Int | Ty::IntLit => Some((i64::MIN as i128, i64::MAX as i128)),
            Ty::Num(n) if n != Num::F64 => {
                let b = n.bits();
                Some(if n.signed() {
                    (-(1i128 << (b - 1)), (1i128 << (b - 1)) - 1)
                } else {
                    (0, (1i128 << b) - 1)
                })
            }
            _ => None,
        }
    }

    /// Bits of an integer type.
    fn bits(self) -> u32 {
        match self {
            Ty::Num(n) => n.bits(),
            _ => 64,
        }
    }

    fn signed(self) -> bool {
        !matches!(self, Ty::Num(n) if !n.signed())
    }
}

/// What is known about a value at compile time (R8: literals and `const`).
#[derive(Clone, Copy, PartialEq, Debug)]
enum Val {
    /// An integer computed exactly, not yet checked against the range of its type.
    Int(i128),
    /// A `f64` computed in IEEE 754 (R29).
    Float(f64),
    /// Some other value that is fixed at compile time.
    Known,
    Runtime,
}

const LATER_TYPES: &[&str] = &["f32"];
const BUILTINS: &[&str] = &["print", "range", "repeat", "expect", "assert"];

/// Longest chain of global consts that use each other.
const MAX_CONST_DEPTH: u32 = 256;

/// Constant `int` values by expression span (byte offsets), for the C emitter.
pub type Folded = HashMap<(u32, u32), i64>;

/// The body of a `struct` or `enum` type, with resolved types.
pub enum TypeBody {
    Struct(Vec<(String, Ty)>),
    Enum(Vec<(String, Vec<(Ty, String)>)>),
}

pub struct TypeDef {
    pub name: String,
    pub body: TypeBody,
}

/// What the C emitter takes from the checker, so it never derives types again.
pub struct Info {
    pub folded: Folded,
    /// Type of every checked expression, by span (byte offsets).
    pub exprs: HashMap<(u32, u32), Ty>,
    /// `Ty::Named(i)` is `types[i]`.
    pub types: Vec<TypeDef>,
    /// `Ty::Opt(i)` is `?opts[i]`.
    pub opts: Vec<Ty>,
    /// `Ty::Res(i)` is `ress[i].0 ! ress[i].1`.
    pub ress: Vec<(Ty, Ty)>,
    /// The program uses a form of part 2b (results, optionals, `try`, tests).
    pub part2b: bool,
    /// `Ty::List(i)` is `[]lists[i]`.
    pub lists: Vec<Ty>,
    /// `Ty::Arr(i)` is `[arrs[i].1]arrs[i].0`.
    pub arrs: Vec<(Ty, u64)>,
    /// `Ty::Map(i)` is `map[maps[i].0]maps[i].1`.
    pub maps: Vec<(Ty, Ty)>,
    /// The program uses a form of part 2c (collections, number widths, f64, rune, std
    /// modules); the C backend rejects it until it is implemented.
    pub part2c: bool,
    /// `Ty::Func(i)` is `ftys[i].0(ftys[i].1)`: return type, parameter types with `mut`.
    pub ftys: Vec<(Ty, Vec<(Ty, bool)>)>,
    /// Function literals and blocks by span: their type and the names they capture.
    pub lits: HashMap<(u32, u32), LitInfo>,
    /// Spans of `Name() { … }` that are a call of a function with a block, not a literal.
    pub lit_calls: HashSet<(u32, u32)>,
    /// Reads of a variable (by span) after which it is never read again: the backend moves
    /// them instead of copying (`live::moves`).
    pub moves: HashSet<(u32, u32)>,
    /// Hints (`dif check --hints`): not errors.
    pub hints: Vec<Diag>,
}

/// A function literal (R26): its function type and the values it captures (name, type).
pub struct LitInfo {
    pub ty: Ty,
    pub captures: Vec<(String, Ty)>,
}

pub fn check(file: &str, src: &str, m: &Module) -> (Vec<Diag>, Info) {
    let mut first = Checker::new(file, src, m, None);
    first.module();
    let derived = derive(&first.fns, &first.facts);
    let mut c = Checker::new(file, src, m, Some(derived));
    c.module();
    let types = c
        .types
        .iter()
        .map(|t| TypeDef {
            name: t.name.to_string(),
            body: match &t.kind {
                Kind::Struct(fs) => {
                    TypeBody::Struct(fs.iter().map(|f| (f.name.to_string(), f.ty)).collect())
                }
                Kind::Enum(vs) => TypeBody::Enum(
                    vs.iter()
                        .map(|v| {
                            let fs = v.fields.iter().map(|(t, n)| (*t, n.to_string()));
                            (v.name.to_string(), fs.collect())
                        })
                        .collect(),
                ),
            },
        })
        .collect();
    let info = Info {
        folded: c.folded,
        exprs: c.exprs,
        types,
        opts: c.opts,
        ress: c.ress,
        part2b: c.uses_2b,
        lists: c.lists,
        arrs: c.arrs,
        maps: c.maps,
        part2c: c.uses_2c,
        ftys: c.ftys,
        lits: c.lits,
        lit_calls: c.lit_calls,
        moves: c.moves,
        hints: c.hints,
    };
    (c.diags, info)
}

/// `void main()` for `dif run`/`dif build` (R1); `dif check` of a library does not need it.
pub fn main_diag(m: &Module) -> Option<Diag> {
    let main = m.decls.iter().find_map(|d| match d {
        Decl::Func(f) if f.name == "main" && f.recv.is_none() => Some(f),
        _ => None,
    });
    let hint = "a program starts in void main() { … }";
    let start = Pos {
        line: 1,
        col: 1,
        offset: 0,
    };
    match main {
        None => Some(Diag::new("R1.no_main", start, "no `main` function").hint(hint)),
        Some(f) if f.ret.name != "void" && !f.ret.name.starts_with("void!") => Some(
            Diag::new(
                "R1.main_signature",
                f.pos,
                "`main` must return void or void!E",
            )
            .hint(hint),
        ),
        Some(f) if !f.params.is_empty() => {
            Some(Diag::new("R1.main_params", f.pos, "`main` takes no parameters").hint(hint))
        }
        Some(_) => None,
    }
}

struct FieldInfo<'a> {
    name: &'a str,
    ty: Ty,
    pos: Pos,
    default: Option<&'a Expr>,
}

struct VariantInfo<'a> {
    name: &'a str,
    fields: Vec<(Ty, &'a str)>,
}

enum Kind<'a> {
    Struct(Vec<FieldInfo<'a>>),
    Enum(Vec<VariantInfo<'a>>),
}

struct TypeInfo<'a> {
    name: &'a str,
    pos: Pos,
    kind: Kind<'a>,
    methods: HashMap<&'a str, usize>,
}

struct ParamInfo<'a> {
    ty: Ty,
    is_mut: bool,
    name: &'a str,
}

/// A function or method, by index (its id).
struct FnInfo<'a> {
    decl: &'a Func,
    params: Vec<ParamInfo<'a>>,
    ret: Ty,
    recv: Option<u32>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Slot {
    Param(usize),
    SelfSlot,
}

/// What a function does directly; `deps` say "if the callee changes its slot,
/// I change mine".
#[derive(Default, Clone)]
struct Facts {
    prints: bool,
    changes: Vec<bool>,
    changes_self: bool,
    deps: Vec<(usize, Slot, Slot)>,
    calls: Vec<usize>,
}

#[derive(Clone)]
struct Derived {
    changes: Vec<bool>,
    changes_self: bool,
    effect: bool,
    /// Functions reachable through calls.
    reaches: Vec<bool>,
}

fn derive(fns: &[FnInfo], facts: &[Facts]) -> Vec<Derived> {
    let mut d: Vec<Derived> = facts
        .iter()
        .map(|f| Derived {
            changes: f.changes.clone(),
            changes_self: f.changes_self,
            effect: false,
            reaches: vec![false; facts.len()],
        })
        .collect();
    for (i, f) in facts.iter().enumerate() {
        for &c in &f.calls {
            d[i].reaches[c] = true;
        }
    }
    for k in 0..d.len() {
        for i in 0..d.len() {
            if d[i].reaches[k] {
                for j in 0..d.len() {
                    if d[k].reaches[j] {
                        d[i].reaches[j] = true;
                    }
                }
            }
        }
    }
    loop {
        let mut changed = false;
        for (i, f) in facts.iter().enumerate() {
            for &(callee, from, to) in &f.deps {
                let src = match from {
                    Slot::Param(j) => d[callee].changes.get(j).copied().unwrap_or(false),
                    Slot::SelfSlot => fns[callee].decl.mut_self.is_some(),
                };
                let tgt = match to {
                    Slot::Param(k) => &mut d[i].changes[k],
                    Slot::SelfSlot => &mut d[i].changes_self,
                };
                if src && !*tgt {
                    *tgt = true;
                    changed = true;
                }
            }
            let mut_change = fns[i]
                .params
                .iter()
                .zip(&d[i].changes)
                .any(|(p, c)| p.is_mut && *c);
            let effect = f.prints
                || fns[i].decl.mut_self.is_some()
                || mut_change
                || f.calls.iter().any(|&c| d[c].effect);
            if effect && !d[i].effect {
                d[i].effect = true;
                changed = true;
            }
        }
        if !changed {
            return d;
        }
    }
}

enum ConstState<'a> {
    Todo(&'a VarDecl),
    Busy,
    Done(Ty, Val, Pos),
}

struct Local {
    name: String,
    ty: Ty,
    is_const: bool,
    val: Val,
    pos: Pos,
    slot: Option<Slot>,
}

#[derive(Clone, Copy, PartialEq)]
enum Ctx {
    Loop,
    Switch,
}

struct Checker<'a> {
    file: &'a str,
    src: &'a str,
    m: &'a Module,
    types: Vec<TypeInfo<'a>>,
    type_ids: HashMap<&'a str, u32>,
    fns: Vec<FnInfo<'a>>,
    funcs: HashMap<&'a str, usize>,
    consts: HashMap<&'a str, ConstState<'a>>,
    scopes: Vec<Vec<Local>>,
    /// The function being checked, by id.
    cur: Option<usize>,
    ctx: Vec<Ctx>,
    /// Unknown names already reported: each once.
    unknown: Vec<String>,
    const_depth: u32,
    const_too_deep: bool,
    in_default: bool,
    last_callee: Option<usize>,
    facts: Vec<Facts>,
    derived: Option<Vec<Derived>>,
    call_changes: CallChanges,
    /// `switch` statements (by offset) that are complete or already reported (R10).
    complete: HashSet<u32>,
    /// Names declared in the function being checked (for lost changes, R6).
    fn_locals: HashSet<String>,
    diags: Vec<Diag>,
    folded: Folded,
    exprs: HashMap<(u32, u32), Ty>,
    opts: Vec<Ty>,
    ress: Vec<(Ty, Ty)>,
    lists: Vec<Ty>,
    arrs: Vec<(Ty, u64)>,
    maps: Vec<(Ty, Ty)>,
    uses_2c: bool,
    /// Standard modules brought in by `use` (R16).
    imports: Vec<&'a str>,
    /// The type of the place an expression is read for (a list literal, `repeat`).
    place_ty: Ty,
    /// Hints for a lost change at a position (a `pop` whose list is never read again).
    lost_hints: HashMap<u32, String>,
    moves: HashSet<(u32, u32)>,
    hints: Vec<Diag>,
    /// Reading the place of an assignment (a map entry in it is R30).
    assigning: bool,
    /// A builtin method or function called last: does it have an effect (R13)?
    last_effect: Option<bool>,
    /// A literal-only expression is read for a `f64` place: `7 / 2` is `3.5` (R8).
    lit_float: bool,
    /// Inside a `test` block (R18).
    in_test: bool,
    uses_2b: bool,
    /// Names of the tests seen so far, for duplicates.
    tests: Vec<(&'a str, Pos)>,
    ftys: Vec<(Ty, Vec<(Ty, bool)>)>,
    /// A function type is allowed where a parameter type is read.
    fn_ok: bool,
    /// The function literals being checked, innermost last.
    frames: Vec<LitFrame>,
    lits: HashMap<(u32, u32), LitInfo>,
    lit_calls: HashSet<(u32, u32)>,
    /// Calls (by span) that pass a literal or a function with an effect (R13).
    call_lit: HashSet<(u32, u32)>,
    /// Effect of the literal checked last.
    last_lit_effect: bool,
    /// The return type of the literal that `map` is about to check (its result type, §4.7).
    preset_ret: Option<Ty>,
    /// Calls whose type mismatch is reported at the method name (`map`, §4.7).
    name_pos: HashMap<(u32, u32), Pos>,
}

/// A function literal being checked: names declared outside it are captured (R26).
struct LitFrame {
    ret: Ty,
    /// `scopes.len()` before its parameters.
    base: usize,
    captured: Vec<(String, Ty)>,
    declared: Vec<String>,
    changed: Vec<String>,
    effect: bool,
}

impl<'a> Checker<'a> {
    fn new(file: &'a str, src: &'a str, m: &'a Module, derived: Option<Vec<Derived>>) -> Self {
        Checker {
            file,
            src,
            m,
            types: Vec::new(),
            type_ids: HashMap::new(),
            fns: Vec::new(),
            funcs: HashMap::new(),
            consts: HashMap::new(),
            scopes: Vec::new(),
            cur: None,
            ctx: Vec::new(),
            unknown: Vec::new(),
            const_depth: 0,
            const_too_deep: false,
            in_default: false,
            last_callee: None,
            facts: Vec::new(),
            derived,
            call_changes: HashMap::new(),
            complete: HashSet::new(),
            fn_locals: HashSet::new(),
            diags: Vec::new(),
            folded: HashMap::new(),
            exprs: HashMap::new(),
            opts: Vec::new(),
            ress: Vec::new(),
            lists: Vec::new(),
            arrs: Vec::new(),
            maps: Vec::new(),
            uses_2c: false,
            imports: Vec::new(),
            place_ty: Ty::Err,
            lost_hints: HashMap::new(),
            moves: HashSet::new(),
            hints: Vec::new(),
            assigning: false,
            last_effect: None,
            lit_float: false,
            in_test: false,
            uses_2b: false,
            tests: Vec::new(),
            ftys: Vec::new(),
            fn_ok: false,
            frames: Vec::new(),
            lits: HashMap::new(),
            lit_calls: HashSet::new(),
            call_lit: HashSet::new(),
            last_lit_effect: false,
            preset_ret: None,
            name_pos: HashMap::new(),
        }
    }

    fn err(&mut self, code: &'static str, pos: Pos, msg: impl Into<String>, hint: Option<String>) {
        let mut d = Diag::new(code, pos, msg);
        d.hint = hint;
        self.diags.push(d);
    }

    fn text(&self, e: &Expr) -> &'a str {
        self.src
            .get(e.pos.offset as usize..e.end as usize)
            .unwrap_or("")
    }

    fn span(&self, from: u32, to: u32) -> &'a str {
        self.src.get(from as usize..to as usize).unwrap_or("")
    }

    fn at(&self, p: Pos) -> String {
        format!("{}:{}:{}", self.file, p.line, p.col)
    }

    fn tn(&self, t: Ty) -> String {
        match t {
            Ty::Int | Ty::IntLit => "int".into(),
            Ty::Bool => "bool".into(),
            Ty::Str => "str".into(),
            Ty::Void => "void".into(),
            Ty::Num(n) => n.name().into(),
            Ty::FloatLit => "f64".into(),
            Ty::Rune => "rune".into(),
            Ty::Named(id) => self.types[id as usize].name.into(),
            Ty::Opt(i) => format!("?{}", self.tn(self.opts[i as usize])),
            Ty::Res(i) => {
                let (t, e) = self.ress[i as usize];
                format!("{}!{}", self.tn(t), self.tn(e))
            }
            Ty::List(i) => format!("[]{}", self.tn_elem(self.lists[i as usize])),
            Ty::Arr(i) => {
                let (t, n) = self.arrs[i as usize];
                format!("[{n}]{}", self.tn_elem(t))
            }
            Ty::Map(i) => {
                let (k, v) = self.maps[i as usize];
                format!("map[{}]{}", self.tn_elem(k), self.tn_elem(v))
            }
            Ty::Func(i) => {
                let (r, ps) = &self.ftys[i as usize];
                let ps: Vec<String> = ps
                    .iter()
                    .map(|(t, m)| format!("{}{}", if *m { "mut " } else { "" }, self.tn(*t)))
                    .collect();
                format!("{}({})", self.tn(*r), ps.join(", "))
            }
            Ty::Err => "?".into(),
        }
    }

    /// An element type: a result needs parentheses (`[](int!E)`).
    fn tn_elem(&self, t: Ty) -> String {
        match t {
            Ty::Res(_) => format!("({})", self.tn(t)),
            _ => self.tn(t),
        }
    }

    fn zero(&self, t: Ty) -> &'static str {
        match t {
            Ty::Int | Ty::IntLit => "0",
            Ty::Num(Num::F64) | Ty::FloatLit => "0.0",
            Ty::Num(_) => "0",
            Ty::Bool => "false",
            Ty::Str => "\"\"",
            Ty::Rune => "'a'",
            Ty::List(_) => "[]",
            Ty::Map(_) => "{}",
            Ty::Opt(_) => "none",
            Ty::Res(i) => self.zero(self.ress[i as usize].0),
            _ => "…",
        }
    }

    /// Does the type use a form of part 2c?
    fn is_2c(&self, t: Ty) -> bool {
        match t {
            Ty::Num(_) | Ty::Rune | Ty::FloatLit | Ty::List(_) | Ty::Arr(_) | Ty::Map(_) => true,
            Ty::Opt(i) => self.is_2c(self.opts[i as usize]),
            Ty::Res(i) => {
                let (a, b) = self.ress[i as usize];
                self.is_2c(a) || self.is_2c(b)
            }
            _ => false,
        }
    }

    /// `?t`; an already reported type stays reported.
    fn opt(&mut self, t: Ty) -> Ty {
        if t == Ty::Err {
            return t;
        }
        self.uses_2b = true;
        let i = self.opts.iter().position(|&x| x == t).unwrap_or_else(|| {
            self.opts.push(t);
            self.opts.len() - 1
        });
        Ty::Opt(i as u32)
    }

    /// `t!e`.
    fn res(&mut self, t: Ty, e: Ty) -> Ty {
        if t == Ty::Err || e == Ty::Err {
            return Ty::Err;
        }
        self.uses_2b = true;
        let i = self
            .ress
            .iter()
            .position(|&x| x == (t, e))
            .unwrap_or_else(|| {
                self.ress.push((t, e));
                self.ress.len() - 1
            });
        Ty::Res(i as u32)
    }

    /// `[]t`.
    fn list(&mut self, t: Ty) -> Ty {
        if t == Ty::Err {
            return t;
        }
        self.uses_2c = true;
        let i = self.lists.iter().position(|&x| x == t).unwrap_or_else(|| {
            self.lists.push(t);
            self.lists.len() - 1
        });
        Ty::List(i as u32)
    }

    /// `r(ps)`.
    fn func_ty(&mut self, r: Ty, ps: Vec<(Ty, bool)>) -> Ty {
        if r == Ty::Err || ps.iter().any(|p| p.0 == Ty::Err) {
            return Ty::Err;
        }
        let i = (self.ftys.iter())
            .position(|x| x.0 == r && x.1 == ps)
            .unwrap_or_else(|| {
                self.ftys.push((r, ps));
                self.ftys.len() - 1
            });
        Ty::Func(i as u32)
    }

    /// `[n]t`.
    fn arr(&mut self, t: Ty, n: u64) -> Ty {
        if t == Ty::Err {
            return t;
        }
        self.uses_2c = true;
        let i = self
            .arrs
            .iter()
            .position(|&x| x == (t, n))
            .unwrap_or_else(|| {
                self.arrs.push((t, n));
                self.arrs.len() - 1
            });
        Ty::Arr(i as u32)
    }

    /// `map[k]v`.
    fn map(&mut self, k: Ty, v: Ty) -> Ty {
        if k == Ty::Err || v == Ty::Err {
            return Ty::Err;
        }
        self.uses_2c = true;
        let i = self
            .maps
            .iter()
            .position(|&x| x == (k, v))
            .unwrap_or_else(|| {
                self.maps.push((k, v));
                self.maps.len() - 1
            });
        Ty::Map(i as u32)
    }

    /// The `T` of `?T` and `T!E`.
    fn inner(&self, t: Ty) -> Option<Ty> {
        match t {
            Ty::Opt(i) => Some(self.opts[i as usize]),
            Ty::Res(i) => Some(self.ress[i as usize].0),
            _ => None,
        }
    }

    /// Does a value of type `got` fit where `want` is expected? A `T` fits `?T` and
    /// `T!E`, and so `?T!E` (R31).
    fn fits_place(&self, want: Ty, got: Ty) -> bool {
        want == got
            || (got != Ty::Void && self.inner(want).is_some_and(|w| self.fits_place(w, got)))
    }

    /// Functions that end without a value: `void` and `void!E`.
    fn no_value(&self, t: Ty) -> bool {
        t == Ty::Void || self.inner(t) == Some(Ty::Void) && matches!(t, Ty::Res(_))
    }

    /// Struct and enum types that a type holds by value (a list or map keeps its
    /// elements in its own memory, an array inline: R5).
    fn named_in(&self, t: Ty, out: &mut Vec<u32>) {
        match t {
            Ty::Named(id) => out.push(id),
            Ty::Opt(i) => self.named_in(self.opts[i as usize], out),
            Ty::Res(i) => {
                let (a, b) = self.ress[i as usize];
                self.named_in(a, out);
                self.named_in(b, out);
            }
            Ty::Arr(i) => self.named_in(self.arrs[i as usize].0, out),
            _ => {}
        }
    }

    fn is_struct(&self, t: Ty) -> bool {
        matches!(t, Ty::Named(id) if matches!(self.types[id as usize].kind, Kind::Struct(_)))
    }

    fn enum_id(&self, name: &str) -> Option<u32> {
        let id = *self.type_ids.get(name)?;
        matches!(self.types[id as usize].kind, Kind::Enum(_)).then_some(id)
    }

    // ---- module ----

    fn module(&mut self) {
        let m = self.m;
        // 0. `use std.…` lines (R16)
        let mut other = false;
        for d in &m.decls {
            match d {
                Decl::Use(u) => self.use_decl(u, other),
                _ => other = true,
            }
        }
        // 1. the names of all types, so fields and signatures can use them in any order
        let mut first = vec![false; m.decls.len()];
        for (k, d) in m.decls.iter().enumerate() {
            let (name, pos) = match d {
                Decl::Struct(s) => (s.name.as_str(), s.pos),
                Decl::Enum(e) => (e.name.as_str(), e.pos),
                _ => continue,
            };
            if self.new_module_name(name, pos) {
                let kind = match d {
                    Decl::Struct(_) => Kind::Struct(Vec::new()),
                    _ => Kind::Enum(Vec::new()),
                };
                self.type_ids.insert(name, self.types.len() as u32);
                self.types.push(TypeInfo {
                    name,
                    pos,
                    kind,
                    methods: HashMap::new(),
                });
                first[k] = true;
            }
        }
        // 2. fields and variants
        for (k, d) in m.decls.iter().enumerate() {
            if !first[k] {
                continue;
            }
            match d {
                Decl::Struct(s) => self.struct_fields(s),
                Decl::Enum(e) => self.enum_variants(e),
                _ => {}
            }
        }
        // 3. functions, methods and consts
        for (k, d) in m.decls.iter().enumerate() {
            match d {
                Decl::Func(f) if f.recv.is_some() => first[k] = self.add_method(f),
                Decl::Func(f) if self.new_module_name(&f.name, f.pos) => {
                    let id = self.add_fn(f, None);
                    self.funcs.insert(&f.name, id);
                    first[k] = true;
                }
                Decl::Const(v) if self.new_module_name(&v.name, v.pos) => {
                    self.consts.insert(&v.name, ConstState::Todo(v));
                    first[k] = true;
                }
                Decl::Test(_) => first[k] = true,
                _ => {}
            }
        }
        self.containment();
        // 4. bodies
        self.defaults();
        for (k, d) in m.decls.iter().enumerate() {
            if !first[k] {
                continue;
            }
            match d {
                Decl::Const(v) => {
                    let _ = self.global_const(&v.name);
                }
                Decl::Func(f) => self.func(f),
                Decl::Test(t) => self.test(t),
                _ => {}
            }
        }
    }

    /// A new name at module level: not predefined, not declared before.
    fn new_module_name(&mut self, name: &str, pos: Pos) -> bool {
        if self.predefined(name, pos) {
            return false;
        }
        if let Some(prev) = self.module_pos(name) {
            self.already(name, pos, prev);
            return false;
        }
        true
    }

    fn struct_fields(&mut self, s: &'a StructDecl) {
        let mut fields: Vec<FieldInfo<'a>> = Vec::new();
        for f in &s.fields {
            let ty = self.ty(&f.ty, false);
            if let Some(prev) = fields.iter().find(|x| x.name == f.name) {
                let p = prev.pos;
                self.already(&f.name, f.pos, p);
                continue;
            }
            fields.push(FieldInfo {
                name: &f.name,
                ty,
                pos: f.pos,
                default: f.default.as_ref(),
            });
        }
        let id = self.type_ids[s.name.as_str()] as usize;
        self.types[id].kind = Kind::Struct(fields);
    }

    fn enum_variants(&mut self, e: &'a EnumDecl) {
        let mut variants: Vec<VariantInfo<'a>> = Vec::new();
        let mut seen: Vec<(&str, Pos)> = Vec::new();
        for v in &e.variants {
            if let Some((_, p)) = seen.iter().find(|(n, _)| *n == v.name) {
                let p = *p;
                self.already(&v.name, v.pos, p);
                continue;
            }
            seen.push((&v.name, v.pos));
            let fields = v
                .fields
                .iter()
                .map(|f| (self.ty(&f.ty, false), f.name.as_str()))
                .collect();
            variants.push(VariantInfo {
                name: &v.name,
                fields,
            });
        }
        let id = self.type_ids[e.name.as_str()] as usize;
        self.types[id].kind = Kind::Enum(variants);
    }

    fn add_fn(&mut self, f: &'a Func, recv: Option<u32>) -> usize {
        let params = f
            .params
            .iter()
            .map(|p| ParamInfo {
                ty: self.param_ty(&p.ty),
                is_mut: p.mut_pos.is_some(),
                name: &p.name,
            })
            .collect::<Vec<_>>();
        let ret = self.ty(&f.ret, true);
        self.facts.push(Facts {
            changes: vec![false; params.len()],
            ..Facts::default()
        });
        self.fns.push(FnInfo {
            decl: f,
            params,
            ret,
            recv,
        });
        self.fns.len() - 1
    }

    fn add_method(&mut self, f: &'a Func) -> bool {
        let Some(recv) = &f.recv else { return false };
        let Some(&tid) = self.type_ids.get(recv.name.as_str()) else {
            let msg = format!("`{}` is not a struct or enum of this module", recv.name);
            let hint = "methods are declared for struct and enum types of this module".to_string();
            self.err("R7.method_receiver", recv.pos, msg, Some(hint));
            return false;
        };
        let t = &self.types[tid as usize];
        let field_pos = match &t.kind {
            Kind::Struct(fs) => fs.iter().find(|x| x.name == f.name).map(|x| x.pos),
            Kind::Enum(_) => None,
        };
        let prev = field_pos.or_else(|| {
            t.methods
                .get(f.name.as_str())
                .map(|&i| self.fns[i].decl.pos)
        });
        if let Some(prev) = prev {
            self.already(&f.name, f.pos, prev);
            return false;
        }
        let id = self.add_fn(f, Some(tid));
        self.types[tid as usize].methods.insert(&f.name, id);
        true
    }

    /// A type that contains itself by value has no size (R5).
    fn containment(&mut self) {
        let n = self.types.len();
        let edges: Vec<Vec<u32>> = self
            .types
            .iter()
            .map(|t| {
                let tys: Vec<Ty> = match &t.kind {
                    Kind::Struct(fs) => fs.iter().map(|f| f.ty).collect(),
                    // a variant keeps its payload in a counted block: no fixed size needed
                    Kind::Enum(_) => Vec::new(),
                };
                let mut ids = Vec::new();
                for t in tys {
                    self.named_in(t, &mut ids);
                }
                ids
            })
            .collect();
        let reaches = |from: u32, to: u32| {
            let mut seen = vec![false; n];
            let mut stack = edges[from as usize].clone();
            while let Some(x) = stack.pop() {
                if x == to {
                    return true;
                }
                if !std::mem::replace(&mut seen[x as usize], true) {
                    stack.extend(&edges[x as usize]);
                }
            }
            false
        };
        for t in 0..n as u32 {
            // reported once per cycle, at its first type
            let earlier = (0..t).any(|u| reaches(t, u) && reaches(u, t));
            if reaches(t, t) && !earlier {
                let (name, pos) = (self.types[t as usize].name, self.types[t as usize].pos);
                let hint = "a value has a fixed size; keep linked values in a list".to_string();
                self.err(
                    "R5.contains_itself",
                    pos,
                    format!("`{name}` contains itself"),
                    Some(hint),
                );
            }
        }
    }

    /// Field defaults: constants and functions without effect (R5).
    fn defaults(&mut self) {
        let list: Vec<(Ty, &'a Expr)> = self
            .types
            .iter()
            .flat_map(|t| match &t.kind {
                Kind::Struct(fs) => fs
                    .iter()
                    .filter_map(|f| f.default.map(|d| (f.ty, d)))
                    .collect(),
                Kind::Enum(_) => Vec::new(),
            })
            .collect();
        for (ty, d) in list {
            self.in_default = true;
            self.at_place(ty, d, None);
            self.in_default = false;
        }
    }

    fn cycle(&mut self, name: &str, pos: Pos) {
        let msg = format!("`{name}` depends on itself");
        let hint = "give it a value without itself".to_string();
        self.err("R4.depends_on_itself", pos, msg, Some(hint));
    }

    fn module_pos(&self, name: &str) -> Option<Pos> {
        if let Some(&id) = self.funcs.get(name) {
            return Some(self.fns[id].decl.pos);
        }
        if let Some(&id) = self.type_ids.get(name) {
            return Some(self.types[id as usize].pos);
        }
        match self.consts.get(name) {
            Some(ConstState::Todo(v)) => Some(v.pos),
            Some(ConstState::Done(_, _, p)) => Some(*p),
            _ => None,
        }
    }

    fn predefined(&mut self, name: &str, pos: Pos) -> bool {
        let predefined = BUILTINS.contains(&name)
            || LATER_TYPES.contains(&name)
            || matches!(name, "int" | "bool" | "str" | "rune" | "map")
            || Num::ALL.iter().any(|n| n.name() == name)
            || self.imports.contains(&name);
        if predefined {
            let hint = "choose another name".to_string();
            self.err(
                "R4.predefined_name",
                pos,
                format!("`{name}` is a predefined name"),
                Some(hint),
            );
        }
        predefined
    }

    fn already(&mut self, name: &str, pos: Pos, prev: Pos) {
        let msg = format!("`{name}` is already declared at {}", self.at(prev));
        let hint = "names are not redeclared inside their block; choose another name".to_string();
        self.err("R4.already_declared", pos, msg, Some(hint));
    }

    /// A global `const`, checked on first use so the order of declarations does not
    /// matter; `None` while it is being checked (it depends on itself).
    fn global_const(&mut self, name: &str) -> Option<(Ty, Val)> {
        let (key, state) = self.consts.remove_entry(name)?;
        match state {
            ConstState::Done(t, v, p) => {
                self.consts.insert(key, ConstState::Done(t, v, p));
                Some((t, v))
            }
            ConstState::Busy => {
                self.consts.insert(key, ConstState::Busy);
                None
            }
            ConstState::Todo(d) if self.const_depth >= MAX_CONST_DEPTH => {
                // each const checks the ones it uses first: a long chain is deep recursion
                self.consts.insert(key, ConstState::Todo(d));
                if !self.const_too_deep {
                    self.const_too_deep = true;
                    let msg =
                        format!("`{key}` depends on more than {MAX_CONST_DEPTH} consts in a row");
                    self.err(
                        "R4.const_chain",
                        d.pos,
                        msg,
                        Some("compute it in fewer steps".into()),
                    );
                }
                Some((Ty::Err, Val::Known))
            }
            ConstState::Todo(d) => {
                self.consts.insert(key, ConstState::Busy);
                let scopes = std::mem::take(&mut self.scopes);
                let cur = self.cur.take();
                let ty = self.ty(&d.ty, false);
                self.const_depth += 1;
                let (t, v) = self.at_place(ty, &d.value, None);
                self.const_depth -= 1;
                self.scopes = scopes;
                self.cur = cur;
                let mut v = v;
                if t == Ty::Err {
                    v = Val::Known; // reported; its users need no second message
                } else if v == Val::Runtime {
                    let msg = "a global const needs a value known at compile time";
                    self.err("R4.const_runtime", d.value.pos, msg, None);
                    v = Val::Known;
                }
                let v = self.mask(ty, v);
                self.consts.insert(key, ConstState::Done(ty, v, d.pos));
                Some((ty, v))
            }
        }
    }

    /// The header of a function with `mut` added to or removed from one parameter
    /// (`None`: from `self`).
    fn signature(&self, id: usize, toggle: Option<usize>) -> String {
        let f = self.fns[id].decl;
        let params: Vec<String> = f
            .params
            .iter()
            .enumerate()
            .map(|(k, p)| {
                let m = p.mut_pos.is_some() != (Some(k) == toggle);
                format!("{}{} {}", if m { "mut " } else { "" }, p.ty.name, p.name)
            })
            .collect();
        let params: Vec<String> = if f.mut_self.is_some() != toggle.is_none() {
            ["mut self".to_string()].into_iter().chain(params).collect()
        } else {
            params
        };
        let recv = f
            .recv
            .as_ref()
            .map(|r| format!("{}.", r.name))
            .unwrap_or_default();
        format!("{} {recv}{}({})", f.ret.name, f.name, params.join(", "))
    }

    /// The call is inside the recursion cycle of its callee: the outer call reports.
    fn in_cycle(&self, callee: Option<usize>) -> bool {
        match (&self.derived, self.cur, callee) {
            (Some(d), Some(cur), Some(id)) => d[id].reaches[cur],
            _ => false,
        }
    }

    /// The hint of a void call without effect: if it only changes its own copy, take it as mut.
    fn no_effect_hint(&self) -> String {
        let fix = match (&self.derived, self.last_callee) {
            (Some(d), Some(id)) => {
                let f = &self.fns[id];
                if d[id].changes_self && f.decl.mut_self.is_none() {
                    Some(None)
                } else {
                    (0..f.params.len())
                        .find(|&k| d[id].changes[k] && !f.params[k].is_mut)
                        .map(Some)
                }
                .map(|k| self.signature(id, k))
            }
            _ => None,
        };
        match fix {
            Some(s) => format!("take it as mut: {s}"),
            None => "remove the call, or return a result and use it".to_string(),
        }
    }

    /// Does copying a value of type `t` cost something (counted memory inside)?
    fn counted(&self, t: Ty, depth: u32) -> bool {
        if depth > 6 {
            return false;
        }
        match t {
            Ty::Str | Ty::List(_) | Ty::Map(_) => true,
            Ty::Opt(i) => self.counted(self.opts[i as usize], depth + 1),
            Ty::Res(i) => {
                let (v, e) = self.ress[i as usize];
                self.counted(v, depth + 1) || self.counted(e, depth + 1)
            }
            Ty::Arr(i) => self.counted(self.arrs[i as usize].0, depth + 1),
            Ty::Named(i) => match &self.types[i as usize].kind {
                Kind::Struct(fs) => fs.iter().any(|f| self.counted(f.ty, depth + 1)),
                Kind::Enum(vs) => vs
                    .iter()
                    .any(|v| v.fields.iter().any(|(t, _)| self.counted(*t, depth + 1))),
            },
            _ => false,
        }
    }

    fn fn_name(&self, id: usize) -> String {
        let f = self.fns[id].decl;
        match &f.recv {
            Some(r) => format!("{}.{}", r.name, f.name),
            None => f.name.clone(),
        }
    }

    fn func(&mut self, f: &'a Func) {
        let Some(id) = self.fns.iter().position(|x| std::ptr::eq(x.decl, f)) else {
            return;
        };
        let ret = self.fns[id].ret;
        self.cur = Some(id);
        self.fn_locals.clear();
        let errors_before = self.diags.len();
        self.scopes.push(Vec::new());
        let params: Vec<(Ty, &str, Pos)> = self.fns[id]
            .params
            .iter()
            .zip(&f.params)
            .map(|(p, d)| (p.ty, p.name, d.pos))
            .collect();
        for (k, (t, name, pos)) in params.into_iter().enumerate() {
            self.declare(name, pos, t, false, Val::Runtime, Some(Slot::Param(k)));
        }
        self.param_defaults(id, f);
        self.block(&f.body);
        self.scopes.pop();
        if !self.no_value(ret) && ret != Ty::Err && !terminates(&f.body, &self.complete) {
            let msg = format!("`{}` does not return a value on every path", f.name);
            let hint = format!("end it with return, e.g. return {}", self.zero(ret));
            self.err("R8.missing_return", f.pos, msg, Some(hint));
        }
        if let Some(d) = &self.derived {
            let changes_self = d[id].changes_self;
            let unneeded: Vec<(usize, Pos)> = f
                .params
                .iter()
                .enumerate()
                .filter_map(|(k, p)| p.mut_pos.filter(|_| !d[id].changes[k]).map(|mp| (k, mp)))
                .collect();
            let copies: Vec<usize> = (0..f.params.len())
                .filter(|&k| {
                    let (p, ty) = (&f.params[k], self.fns[id].params[k].ty);
                    p.mut_pos.is_none() && d[id].changes[k] && self.counted(ty, 0)
                })
                .collect();
            for k in copies {
                let p = &f.params[k];
                let msg = format!(
                    "`{}` is changed here but taken without mut: every call copies it unless the caller passes its last use",
                    p.name
                );
                let hint = format!(
                    "pass the last use (`{0} = {1}({0}, …)`), or take it as mut: {2}",
                    p.name,
                    f.name,
                    self.signature(id, Some(k))
                );
                self.hints
                    .push(Diag::new("H1.param_copy", p.pos, msg).hint(hint));
            }
            for (k, mp) in unneeded {
                let msg = format!("`{}` is never changed; remove mut", f.params[k].name);
                let hint = self.signature(id, Some(k));
                let mut d = Diag::new("R6.mut_unneeded", mp, msg).hint(hint);
                if self.span(mp.offset, mp.offset + 4) == "mut " {
                    d = d.fix(mp.offset, mp.offset + 4, "");
                }
                self.diags.push(d);
            }
            if let Some(mp) = f.mut_self.filter(|_| !changes_self) {
                let hint = self.signature(id, None);
                self.err(
                    "R6.mut_self_unneeded",
                    mp,
                    "`self` is never changed; remove mut",
                    Some(hint),
                );
            }
            // a broken body has its own causes; lost changes there would only add noise
            if self.diags.len() == errors_before {
                self.lost_changes(id, f);
            }
        }
        self.cur = None;
    }

    /// Defaults stand behind all parameters without one and are constants (R25).
    fn param_defaults(&mut self, id: usize, f: &'a Func) {
        let mut first: Option<&str> = None;
        let mut reported = false;
        for (k, p) in f.params.iter().enumerate() {
            let Some(d) = &p.default else {
                if let (Some(n), false) = (first, reported) {
                    let msg = format!(
                        "`{}` has no default but follows `{n}`, which has one",
                        p.name
                    );
                    let hint = "put parameters with a default last".to_string();
                    self.err("R25.default_last", p.pos, msg, Some(hint));
                    reported = true;
                }
                continue;
            };
            first.get_or_insert(&p.name);
            let ty = self.fns[id].params[k].ty;
            let why = if p.mut_pos.is_some() {
                "a `mut` parameter has no default"
            } else if matches!(ty, Ty::Func(_)) {
                "a function parameter has no default"
            } else {
                let (t, v) = self.at_place(ty, d, None);
                if t != Ty::Err && v == Val::Runtime {
                    let hint = "pass it at the call, or use a constant: const int SIZE = 4";
                    self.err(
                        "R25.default_constant",
                        d.pos,
                        "default must be a constant",
                        Some(hint.into()),
                    );
                }
                continue;
            };
            self.err("R25.default_not_allowed", d.pos, why, None);
        }
    }

    fn lost_changes(&mut self, id: usize, f: &'a Func) {
        let mut escape: Vec<String> = f
            .params
            .iter()
            .filter(|p| p.mut_pos.is_some())
            .map(|p| p.name.clone())
            .collect();
        if f.mut_self.is_some() {
            escape.push("self".into());
        }
        self.moves
            .extend(live::moves(&f.body, &self.call_changes, &escape));
        for (place, pos) in live::lost(&f.body, &self.call_changes, &escape) {
            let root = place.split('.').next().unwrap_or("").to_string();
            if root != "self" && !self.fn_locals.contains(&root) {
                continue; // a const, a function or an unknown name: reported elsewhere
            }
            let slot = if root == "self" && f.mut_self.is_none() {
                Some(None)
            } else {
                f.params.iter().position(|p| p.name == root).map(Some)
            };
            match slot {
                Some(k) => {
                    let msg = format!("the change to `{place}` is lost: `{root}` is a copy");
                    let hint = format!("take it as mut: {}", self.signature(id, k));
                    self.err("R6.lost_copy", pos, msg, Some(hint));
                }
                None => {
                    let msg = format!("`{place}` is changed but never read afterwards");
                    let hint = self
                        .lost_hints
                        .get(&pos.offset)
                        .cloned()
                        .unwrap_or_else(|| "use the value, or remove the change".to_string());
                    self.err("R6.dead_change", pos, msg, Some(hint));
                }
            }
        }
    }

    // ---- names ----

    fn declare(
        &mut self,
        name: &str,
        pos: Pos,
        ty: Ty,
        is_const: bool,
        val: Val,
        slot: Option<Slot>,
    ) {
        if self.predefined(name, pos) {
            // the name exists anyway, so its uses add no second message
            if let Some(s) = self.scopes.last_mut() {
                s.push(Local {
                    name: name.to_string(),
                    ty: Ty::Err,
                    is_const: false,
                    val: Val::Runtime,
                    pos,
                    slot: None,
                });
            }
            return;
        }
        let prev = self
            .scopes
            .iter()
            .flatten()
            .find(|l| l.name == name)
            .map(|l| l.pos)
            .or_else(|| self.module_pos(name));
        if let Some(prev) = prev {
            self.already(name, pos, prev);
            return;
        }
        let local = Local {
            name: name.to_string(),
            ty,
            is_const,
            val: if is_const { val } else { Val::Runtime },
            pos,
            slot,
        };
        if self.cur.is_some() || self.in_test {
            self.fn_locals.insert(name.to_string());
        }
        if let Some(fr) = self.frames.last_mut() {
            fr.declared.push(name.to_string());
        }
        if let Some(s) = self.scopes.last_mut() {
            s.push(local);
        }
    }

    fn local(&self, name: &str) -> Option<&Local> {
        self.scopes.iter().rev().flatten().find(|l| l.name == name)
    }

    /// All names visible here, for "did you mean".
    fn visible(&self) -> Vec<&str> {
        let mut v: Vec<&str> = self
            .scopes
            .iter()
            .flatten()
            .map(|l| l.name.as_str())
            .collect();
        v.extend(self.funcs.keys().copied());
        v.extend(self.consts.keys().copied());
        v.push("print");
        v
    }

    fn unknown_name(&mut self, name: &str, pos: Pos) {
        if self.unknown.iter().any(|n| n == name) {
            return;
        }
        self.unknown.push(name.to_string());
        let hint = closest(name, self.visible().into_iter()).map(|c| format!("did you mean {c}?"));
        self.err(
            "R4.unknown_name",
            pos,
            format!("unknown name `{name}`"),
            hint,
        );
    }

    fn lookup(&mut self, name: &str, pos: Pos) -> (Ty, Val) {
        if let Some(l) = self.local(name) {
            let (ty, val) = (l.ty, l.val);
            self.capture(name, ty);
            if let Ty::Func(_) = ty {
                self.only_called(name, pos);
                return (Ty::Err, Val::Runtime);
            }
            return (ty, val);
        }
        if self.consts.contains_key(name) {
            return self.global_const(name).unwrap_or_else(|| {
                self.cycle(name, pos);
                (Ty::Err, Val::Runtime)
            });
        }
        if self.funcs.contains_key(name) || BUILTINS.contains(&name) {
            if self.funcs.contains_key(name) {
                self.only_called(name, pos);
                return (Ty::Err, Val::Runtime);
            }
            let msg = format!("`{name}` is a function; call it: {name}(…)");
            self.err("R8.fn_as_value", pos, msg, None);
            return (Ty::Err, Val::Runtime);
        }
        if self.type_ids.contains_key(name) {
            self.err(
                "R8.type_as_value",
                pos,
                format!("`{name}` is a type, not a value"),
                None,
            );
            return (Ty::Err, Val::Runtime);
        }
        if p2c::STD_MODULES.contains(&name) {
            if self.imports.contains(&name) {
                let msg = format!("`{name}` is a module, not a value");
                self.err(
                    "R8.module_as_value",
                    pos,
                    msg,
                    Some(format!("write {name}.name(…)")),
                );
            } else {
                self.not_imported(name, pos);
            }
            return (Ty::Err, Val::Runtime);
        }
        self.unknown_name(name, pos);
        (Ty::Err, Val::Runtime)
    }

    /// The root of a place (`p` in `p.a.b`): a local or `self`.
    fn root_slot(&self, place: &Expr) -> Option<Slot> {
        match &place.kind {
            ExprKind::Ident(n) => self.local(n).and_then(|l| l.slot),
            ExprKind::SelfRef => Some(Slot::SelfSlot),
            ExprKind::Field(b, _) | ExprKind::Index(b, _, _) => self.root_slot(b),
            _ => None,
        }
    }

    /// Name of the root of a place and whether it is a `const`.
    fn root_of(&self, place: &Expr) -> Option<(String, bool)> {
        match &place.kind {
            ExprKind::Ident(n) => match self.local(n) {
                Some(l) => Some((n.clone(), l.is_const)),
                None => Some((n.clone(), self.consts.contains_key(n.as_str()))),
            },
            ExprKind::SelfRef => Some(("self".into(), false)),
            ExprKind::Field(b, _) | ExprKind::Index(b, _, _) => self.root_of(b),
            _ => None,
        }
    }

    fn changed(&mut self, place: &Expr) {
        if let Some((root, false)) = self.root_of(place) {
            self.lit_changed(&root, place.pos);
        }
        let (Some(id), Some(slot)) = (self.cur, self.root_slot(place)) else {
            return;
        };
        match slot {
            Slot::Param(k) => self.facts[id].changes[k] = true,
            Slot::SelfSlot => self.facts[id].changes_self = true,
        }
    }

    // ---- statements ----

    fn block(&mut self, b: &'a Block) {
        self.scopes.push(Vec::new());
        for s in &b.stmts {
            self.stmt(s);
        }
        self.scopes.pop();
    }

    fn stmt(&mut self, s: &'a Stmt) {
        match s {
            Stmt::Var(v) => {
                let before = self.diags.len();
                let ty = self.ty(&v.ty, false);
                let typed = self.diags.len();
                let (_, val) = self.at_place(ty, &v.value, Some(&v.name));
                if typed > before {
                    // the type was reported: no second message for the value
                    self.diags.truncate(typed);
                }
                let val = self.mask(ty, val);
                self.declare(&v.name, v.pos, ty, v.is_const, val, None);
            }
            Stmt::Assign {
                place,
                op,
                value,
                pos,
            } => self.assign(place, op, value, *pos),
            Stmt::If {
                branches,
                otherwise,
                ..
            } => {
                for (k, (c, b)) in branches.iter().enumerate() {
                    self.cond(c, if k == 0 { "if" } else { "else if" });
                    self.block(b);
                }
                if let Some(b) = otherwise {
                    self.block(b);
                }
            }
            Stmt::While { cond, body, .. } => {
                self.cond(cond, "while");
                self.ctx.push(Ctx::Loop);
                self.block(body);
                self.ctx.pop();
            }
            Stmt::For {
                vars, iter, body, ..
            } => self.for_stmt(vars, iter, body),
            Stmt::Switch {
                subject,
                cases,
                default,
                pos,
            } => self.switch(subject, cases, default.as_ref(), *pos),
            Stmt::Break(p) => match self.ctx.last() {
                Some(Ctx::Switch) => {
                    let hint = "leave the loop after the switch, or return".to_string();
                    self.err(
                        "R11.break_in_switch",
                        *p,
                        "`break` directly inside switch",
                        Some(hint),
                    );
                }
                Some(Ctx::Loop) => {}
                None => self.err("R11.break_outside_loop", *p, "`break` outside a loop", None),
            },
            Stmt::Continue(p) => {
                if !self.ctx.contains(&Ctx::Loop) {
                    self.err(
                        "R11.continue_outside_loop",
                        *p,
                        "`continue` outside a loop",
                        None,
                    );
                }
            }
            Stmt::Return(value, p) => self.ret(value.as_ref(), *p),
            Stmt::Call(e) => {
                let before = self.diags.len();
                self.last_callee = None;
                self.last_effect = None;
                let (t, _) = self.expr(e);
                let effect = match (&self.derived, self.last_callee) {
                    (Some(d), Some(id)) => d[id].effect,
                    _ => self.last_effect.unwrap_or(true),
                };
                let effect = effect || self.lit_effect_in(e);
                if matches!(t, Ty::Opt(_) | Ty::Res(_)) {
                    let text = flat(self.text(e));
                    let msg = format!("result of `{text}` is never used");
                    let hint = format!(
                        "use it with try, ?? or switch, or discard it explicitly with `_ = {text}`"
                    );
                    self.err("R13.unused_result", e.pos, msg, Some(hint));
                } else if t == Ty::Void
                    && !effect
                    && self.diags.len() == before
                    && !matches!(e.kind, ExprKind::Try(..))
                    && !self.in_cycle(self.last_callee)
                {
                    let text = flat(self.text(e));
                    let msg = format!("call of `{text}` has no effect");
                    let hint = self.no_effect_hint();
                    self.err("R13.no_effect", e.pos, msg, Some(hint));
                } else if t != Ty::Void && t != Ty::Err && !effect {
                    let text = flat(self.text(e));
                    let msg = format!("result of `{text}` is never used");
                    let hint = format!("use it, or discard it explicitly with `_ = {text}`");
                    self.err("R13.unused_result", e.pos, msg, Some(hint));
                }
            }
            Stmt::Discard(e, _) => {
                let (t, _) = self.rvalue(e);
                if t == Ty::Void {
                    let msg = format!("`{}` has no result to discard", self.text(e));
                    self.err(
                        "R8.discard_void",
                        e.pos,
                        msg,
                        Some("call it without `_ =`".into()),
                    );
                }
            }
        }
    }

    fn assign(&mut self, place: &'a Expr, op: &str, value: &'a Expr, pos: Pos) {
        if let ExprKind::Ident(name) = &place.kind {
            if self.local(name).is_none() {
                if self.consts.contains_key(name.as_str()) {
                    let decl = self.module_pos(name).unwrap_or(pos);
                    let msg = format!("cannot assign to const `{name}`");
                    let hint = format!("it is declared const at {}", self.at(decl));
                    self.err("R4.assign_const", place.pos, msg, Some(hint));
                } else if self.funcs.contains_key(name.as_str()) {
                    let msg = format!("cannot assign to function `{name}`");
                    self.err("R8.assign_fn", place.pos, msg, None);
                } else {
                    let (t, _) = self.rvalue(value);
                    let ty = if t == Ty::Err { "T".into() } else { self.tn(t) };
                    let hint =
                        format!("declare it with a type: {ty} {name} = {}", self.text(value));
                    self.err(
                        "R2.not_declared",
                        place.pos,
                        format!("`{name}` is not declared"),
                        Some(hint),
                    );
                }
                return;
            }
        }
        if let Some((root, true)) = self.root_of(place) {
            let decl = self
                .local(&root)
                .map(|l| l.pos)
                .or_else(|| self.module_pos(&root))
                .unwrap_or(pos);
            let msg = format!("cannot assign to const `{root}`");
            let hint = format!("it is declared const at {}", self.at(decl));
            self.err("R4.assign_const", place.pos, msg, Some(hint));
            return;
        }
        // `m[k] = v` sets an entry of a map; the entry itself is no place (R30)
        let mut want = Ty::Err;
        let mut known_want = false;
        if let ExprKind::Index(b, idx, bp) = &place.kind {
            let (bt, _) = self.rvalue(b);
            if let Ty::Map(i) = bt {
                self.uses_2c = true;
                let (kt, vt) = self.maps[i as usize];
                self.at_place(kt, idx, None);
                self.exprs.insert((place.pos.offset, place.end), vt);
                self.changed(place);
                if op == "=" {
                    self.at_place(vt, value, None);
                } else {
                    let (rt, _) = self.rvalue(value);
                    let ot = self.opt(vt);
                    if rt != Ty::Err && vt != Ty::Err {
                        let binop = &op[..op.len() - 1];
                        let msg =
                            format!("cannot apply `{op}` to {} and {}", self.tn(ot), self.tn(rt));
                        let hint = if op == "+=" && vt.is_number() {
                            let (m, k, x) = (self.text(b), self.text(idx), self.text(value));
                            format!("{m}.add({k}, {x})")
                        } else {
                            format!(
                                "{p} = ({p} ?? {}) {binop} {}",
                                self.zero(vt),
                                self.text(value),
                                p = self.text(place)
                            )
                        };
                        self.err("R8.binary_types", pos, msg, Some(hint));
                    }
                }
                return;
            }
            (want, _) = self.index_of(place, bt, idx, *bp);
            self.exprs.insert((place.pos.offset, place.end), want);
            known_want = true;
        }
        if !known_want {
            self.assigning = true;
            (want, _) = self.expr(place);
            self.assigning = false;
        }
        if want != Ty::Err && self.map_entry(place) {
            self.map_place_error(place);
            self.rvalue(value);
            return;
        }
        self.changed(place);
        if op == "=" {
            self.at_place(want, value, None);
            return;
        }
        let (rt, rv) = self.expr(value);
        let binop = &op[..op.len() - 1];
        self.arith(binop, op, (place, want, Val::Runtime), (value, rt, rv), pos);
    }

    fn ret(&mut self, value: Option<&'a Expr>, p: Pos) {
        if let Some(fr) = self.frames.last() {
            let want = fr.ret;
            return self.lit_ret(want, value, p);
        }
        if self.in_test {
            if let Some(e) = value {
                self.rvalue(e);
                let msg = "a test returns nothing; remove the value";
                self.err("R8.test_return_value", e.pos, msg, None);
            }
            return;
        }
        let Some(id) = self.cur else { return };
        let (want, name) = (self.fns[id].ret, self.fns[id].decl.name.clone());
        match value {
            None if !self.no_value(want) && want != Ty::Err => {
                let msg = format!("`{name}` returns {}; give `return` a value", self.tn(want));
                let hint = format!("return {}", self.zero(want));
                self.err("R8.return_needs_value", p, msg, Some(hint));
            }
            Some(e) if want == Ty::Void => {
                self.rvalue(e);
                let msg = format!("`{name}` returns nothing; remove the value");
                self.err("R8.return_void_value", e.pos, msg, None);
            }
            Some(e) => {
                self.at_place(want, e, None);
            }
            None => {}
        }
    }

    // ---- switch (R10) ----

    fn switch(
        &mut self,
        subject: &'a Expr,
        cases: &'a [Case],
        default: Option<&'a (Block, Pos)>,
        pos: Pos,
    ) {
        let (t, _) = self.rvalue(subject);
        let over: Option<Ty> = match t {
            t if t.is_int() => Some(t),
            Ty::Rune | Ty::Str | Ty::Err | Ty::Opt(_) | Ty::Res(_) => Some(t),
            Ty::Named(id) if !self.is_struct(t) => Some(Ty::Named(id)),
            _ => {
                let hint = (t == Ty::Bool).then(|| "use if".to_string());
                let msg = format!("cannot switch over {}", self.tn(t));
                self.err("R10.switch_type", subject.pos, msg, hint);
                None
            }
        };
        let mut seen: Vec<(String, Pos)> = Vec::new();
        for case in cases {
            self.scopes.push(Vec::new());
            for pat in &case.pats {
                let Some(over) = over else { continue };
                if let Some((key, p)) = self.pattern(pat, over, case.pats.len()) {
                    match seen.iter().find(|(k, _)| *k == key) {
                        Some((_, first)) => {
                            let msg =
                                format!("case {key} is already handled at {}", self.at(*first));
                            self.err("R10.case_duplicate", p, msg, None);
                        }
                        None => seen.push((key, p)),
                    }
                }
            }
            self.ctx.push(Ctx::Switch);
            self.block(&case.body);
            self.ctx.pop();
            self.scopes.pop();
        }
        if let Some((b, _)) = default {
            self.ctx.push(Ctx::Switch);
            self.block(b);
            self.ctx.pop();
        }
        match over {
            Some(Ty::Named(_) | Ty::Opt(_) | Ty::Res(_)) => {
                let all: Vec<String> = match over {
                    Some(Ty::Named(id)) => match &self.types[id as usize].kind {
                        Kind::Enum(vs) => vs
                            .iter()
                            .map(|v| format!("{}.{}", self.types[id as usize].name, v.name))
                            .collect(),
                        Kind::Struct(_) => Vec::new(),
                    },
                    Some(Ty::Opt(_)) => vec!["some".into(), "none".into()],
                    _ => vec!["ok".into(), "err".into()],
                };
                let missing: Vec<&String> = all
                    .iter()
                    .filter(|v| !seen.iter().any(|(k, _)| k == *v))
                    .collect();
                match (missing.first(), default) {
                    (Some(first), None) => {
                        let list: Vec<&str> = missing.iter().map(|s| s.as_str()).collect();
                        let msg = format!("switch does not handle {}", list.join(", "));
                        let hint = format!("add case {first}: …, or default:");
                        self.err("R10.switch_missing", pos, msg, Some(hint));
                    }
                    (None, Some((_, dpos))) => {
                        let msg = "default is never reached: every variant is handled";
                        self.err(
                            "R10.default_unreachable",
                            *dpos,
                            msg,
                            Some("remove default".into()),
                        );
                    }
                    _ => {}
                }
                self.complete.insert(pos.offset);
            }
            Some(o) if (o.is_int() || matches!(o, Ty::Rune | Ty::Str)) && default.is_none() => {
                let msg = format!("switch over {} needs default", self.tn(t));
                self.err("R10.default_needed", pos, msg, Some("default:".into()));
                self.complete.insert(pos.offset);
            }
            _ => {
                self.complete.insert(pos.offset);
            }
        }
    }

    /// Checks one case value against the type switched over; returns its key for
    /// duplicates and completeness.
    fn pattern(&mut self, pat: &'a Pattern, over: Ty, values: usize) -> Option<(String, Pos)> {
        let wrong = |c: &mut Self, text: String, t: Ty, p: Pos| {
            let msg = format!(
                "case {text} is {}, the switch is over {}",
                c.tn(t),
                c.tn(over)
            );
            c.err("R8.case_type", p, msg, None);
            None
        };
        match pat {
            Pattern::Int(v, p) => match over {
                t if t.is_int() => {
                    let (lo, hi) = t.range().unwrap_or((0, 0));
                    if *v < lo || *v > hi {
                        let msg = format!("case {v} does not fit in {}", self.tn(t));
                        self.err("R8.case_range", *p, msg, None);
                        return None;
                    }
                    Some((v.to_string(), *p))
                }
                Ty::Err => None,
                _ => wrong(self, v.to_string(), Ty::Int, *p),
            },
            Pattern::Rune(c, p) => match over {
                Ty::Rune => Some((format!("{:?}", char::from_u32(*c).unwrap_or('?')), *p)),
                Ty::Err => None,
                _ => wrong(
                    self,
                    format!("{:?}", char::from_u32(*c).unwrap_or('?')),
                    Ty::Rune,
                    *p,
                ),
            },
            Pattern::Str(s, p) => match over {
                Ty::Str => Some((format!("{s:?}"), *p)),
                Ty::Err => None,
                _ => wrong(self, format!("{s:?}"), Ty::Str, *p),
            },
            Pattern::Name(n, p) => {
                if let Ty::Named(id) = over {
                    let is_variant = matches!(&self.types[id as usize].kind,
                        Kind::Enum(vs) if vs.iter().any(|v| v.name == n));
                    if is_variant {
                        let hint = format!("case {}.{n}:", self.types[id as usize].name);
                        self.err(
                            "R10.variant_qualified",
                            *p,
                            "variants are written with their type",
                            Some(hint),
                        );
                        return None;
                    }
                }
                let (t, v) = self.lookup(n, *p);
                if t == Ty::Err || over == Ty::Err {
                    return None;
                }
                if t != over {
                    return wrong(self, n.clone(), t, *p);
                }
                let key = match v {
                    Val::Int(x) => x.to_string(),
                    _ => n.clone(),
                };
                Some((key, *p))
            }
            Pattern::Variant {
                ty,
                name,
                binds,
                pos,
            } => {
                let Some(eid) = self.enum_id(ty) else {
                    let msg = format!("`{ty}` is not an enum of this module");
                    self.err("R4.not_enum", *pos, msg, None);
                    return None;
                };
                let vfields: Vec<(Ty, &'a str)> = match &self.types[eid as usize].kind {
                    Kind::Enum(vs) => match vs.iter().find(|v| v.name == name) {
                        Some(v) => v.fields.clone(),
                        None => {
                            let names: Vec<&str> = vs.iter().map(|v| v.name).collect();
                            let hint = closest(name, names.into_iter())
                                .map(|c| format!("did you mean {ty}.{c}?"));
                            self.err(
                                "R4.no_variant",
                                *pos,
                                format!("`{ty}` has no variant `{name}`"),
                                hint,
                            );
                            return None;
                        }
                    },
                    Kind::Struct(_) => return None,
                };
                let key = format!("{ty}.{name}");
                if over != Ty::Err && over != Ty::Named(eid) {
                    return wrong(self, key, Ty::Named(eid), *pos);
                }
                if let Some(bs) = binds {
                    if values > 1 {
                        let msg = "a case with several values cannot bind values";
                        self.err("R10.case_multi_bind", *pos, msg, None);
                        return Some((key, *pos));
                    }
                    if bs.iter().all(|b| b.is_none()) {
                        let msg = "nothing is bound";
                        self.err("R10.nothing_bound", *pos, msg, Some(format!("case {key}:")));
                        return Some((key, *pos));
                    }
                    let fits = bs.len() == vfields.len()
                        && bs.iter().zip(&vfields).all(|(b, (t, _))| match b {
                            Some(p) => {
                                let bt = self.ty(&p.ty, false);
                                bt == *t || bt == Ty::Err || *t == Ty::Err
                            }
                            None => true,
                        });
                    if !fits {
                        let list: Vec<String> = vfields
                            .iter()
                            .map(|(t, n)| format!("{} {n}", self.tn(*t)))
                            .collect();
                        let msg = format!("`{key}` has the values {}", list.join(", "));
                        self.err("R8.variant_values", *pos, msg, None);
                        // the names still exist, so their uses add no second message
                        for p in bs.iter().flatten() {
                            self.declare(&p.name, p.pos, Ty::Err, false, Val::Runtime, None);
                        }
                        return Some((key, *pos));
                    }
                    for (b, (t, _)) in bs.iter().zip(&vfields) {
                        if let Some(p) = b {
                            self.declare(&p.name, p.pos, *t, false, Val::Runtime, None);
                        }
                    }
                }
                Some((key, *pos))
            }
            Pattern::Result { kind, bind, pos } => {
                self.result_pattern(kind, bind, *pos, over, values)
            }
        }
    }

    /// `ok`, `err`, `some`, `none` over `T!E` and `?T` (R10, R12).
    fn result_pattern(
        &mut self,
        kind: &'static str,
        bind: &'a Option<Option<Param>>,
        pos: Pos,
        over: Ty,
        values: usize,
    ) -> Option<(String, Pos)> {
        self.uses_2b = true;
        let key = (kind.to_string(), pos);
        let names = |bind: &'a Option<Option<Param>>| bind.iter().flatten();
        let skip = |c: &mut Self| {
            // the names still exist, so their uses add no second message
            for p in names(bind) {
                c.declare(&p.name, p.pos, Ty::Err, false, Val::Runtime, None);
            }
        };
        if over == Ty::Err {
            skip(self);
            return None;
        }
        let want_res = matches!(kind, "ok" | "err");
        let value = match over {
            Ty::Res(i) if want_res => Some(self.ress[i as usize]),
            Ty::Opt(i) if !want_res => Some((self.opts[i as usize], Ty::Void)),
            _ => None,
        };
        let Some((ok_t, err_t)) = value else {
            let form = if want_res { "T!E" } else { "?T" };
            let msg = format!(
                "`{kind}` needs a switch over {form}, found {}",
                self.tn(over)
            );
            self.err("R8.switch_wrong_type", pos, msg, None);
            skip(self);
            return None;
        };
        let held = match kind {
            "err" => Some(err_t),
            "none" => None,
            _ => Some(ok_t),
        };
        let Some(b) = bind else {
            return Some(key);
        };
        if values > 1 {
            let msg = "a case with several values cannot bind values";
            self.err("R10.case_multi_bind", pos, msg, None);
            skip(self);
            return Some(key);
        }
        match held {
            None => {
                self.err(
                    "R10.none_no_value",
                    pos,
                    "`none` has no value",
                    Some("case none:".into()),
                );
                skip(self);
            }
            Some(Ty::Void) => {
                let msg = format!("`ok` has no value in {}", self.tn(over));
                self.err("R10.ok_no_value", pos, msg, Some("case ok:".into()));
                skip(self);
            }
            Some(t) => {
                if let Some(p) = b {
                    let bt = self.ty(&p.ty, false);
                    if bt == t || bt == Ty::Err || t == Ty::Err {
                        self.declare(&p.name, p.pos, t, false, Val::Runtime, None);
                    } else {
                        let tn = self.tn(t);
                        let art =
                            if tn.starts_with(['a', 'e', 'i', 'o', 'u', 'A', 'E', 'I', 'O', 'U']) {
                                "an"
                            } else {
                                "a"
                            };
                        let hint = format!("case {kind}({tn} {}):", p.name);
                        self.err(
                            "R8.bind_type",
                            p.ty.pos,
                            format!("`{kind}` holds {art} {tn}"),
                            Some(hint),
                        );
                        skip(self);
                    }
                }
            }
        }
        Some(key)
    }

    // ---- expressions ----

    /// The type the context wants versus the type found (R8).
    fn expect(&mut self, want: Ty, got: Ty, e: &Expr) {
        if !self.fits_place(want, got) && want != Ty::Err && got != Ty::Err {
            let msg = format!("expected {}, found {}", self.tn(want), self.tn(got));
            let hint = self.conv_hint(want, got, e);
            let pos = (self.name_pos.get(&(e.pos.offset, e.end)))
                .copied()
                .unwrap_or(e.pos);
            self.err("R8.type_mismatch", pos, msg, hint);
        }
    }

    /// A condition (R9); returns what is known about its value.
    fn cond(&mut self, e: &'a Expr, ctx: &str) -> Val {
        let (t, v) = self.rvalue(e);
        if t == Ty::Bool || t == Ty::Err {
            return v;
        }
        let text = self.text(e);
        let cmp = match t {
            t if t.is_int() => Some(format!("{text} != 0")),
            Ty::Str => Some(format!("{text} != \"\"")),
            _ => None,
        };
        let tn = self.tn(t);
        let (msg, hint) = match ctx {
            "expect" => (
                format!("condition is {tn}, not bool"),
                cmp.as_ref()
                    .map(|c| format!("compare explicitly: expect({c})")),
            ),
            "assert" => (
                format!("condition is {tn}, not bool"),
                cmp.as_ref()
                    .map(|c| format!("compare explicitly: assert({c})")),
            ),
            "if" | "else if" | "while" => (
                format!("condition is {tn}, not bool"),
                cmp.as_ref()
                    .map(|c| format!("compare explicitly: {ctx} {c}")),
            ),
            op => (
                format!("operand of `{op}` is {tn}, not bool"),
                cmp.as_ref().map(|c| format!("compare explicitly: {c}")),
            ),
        };
        let id = if matches!(ctx, "expect" | "assert" | "if" | "else if" | "while") {
            "R9.condition"
        } else {
            "R9.operand"
        };
        let mut d = Diag::new(id, e.pos, msg);
        d.hint = hint;
        if id == "R9.condition" {
            if let Some(c) = cmp {
                d = d.fix(e.pos.offset, e.end, c);
            }
        }
        self.diags.push(d);
        Val::Runtime
    }

    /// A value in a place where it is used: a literal becomes `int` or `f64`, a constant
    /// must fit its type (R8).
    fn rvalue(&mut self, e: &'a Expr) -> (Ty, Val) {
        let (t, v) = self.expr(e);
        self.finish(t, v, e, None)
    }

    fn out_of_range(&mut self, pos: Pos) {
        let hint = "the value does not fit in int".to_string();
        self.err(
            "R8.int_range",
            pos,
            "integer constant out of range",
            Some(hint),
        );
    }

    fn expr(&mut self, e: &'a Expr) -> (Ty, Val) {
        let place = std::mem::replace(&mut self.place_ty, Ty::Err);
        if matches!(e.kind, ExprKind::Call(..)) && !self.frames.is_empty() {
            self.last_callee = None;
            self.last_effect = None;
        }
        let r = self.expr_kind(e, place);
        if matches!(e.kind, ExprKind::Call(..)) && self.call_effect(e) {
            if let Some(fr) = self.frames.last_mut() {
                fr.effect = true;
            }
        }
        self.exprs.insert((e.pos.offset, e.end), r.0);
        if self.is_2c(r.0) {
            self.uses_2c = true;
        }
        r
    }

    fn expr_kind(&mut self, e: &'a Expr, place: Ty) -> (Ty, Val) {
        match &e.kind {
            ExprKind::Missing => (Ty::Err, Val::Runtime),
            ExprKind::FuncLit(l) => self.func_lit_value(e, l),
            ExprKind::Int(v) if self.lit_float => (Ty::FloatLit, Val::Float(*v as f64)),
            ExprKind::Int(v) => (Ty::IntLit, Val::Int(*v as i128)),
            ExprKind::Float(text) => self.float_lit(e, text),
            ExprKind::Rune(c) => {
                self.uses_2c = true;
                (Ty::Rune, Val::Int(*c as i128))
            }
            ExprKind::ListLit(items) => self.list_lit(e, items, place),
            ExprKind::MapLit(entries) => self.map_lit(e, entries, place),
            ExprKind::Bool(_) => (Ty::Bool, Val::Known),
            ExprKind::Str(segs) => {
                let mut known = true;
                for s in segs {
                    match s {
                        StrSeg::Lit(_) => {}
                        StrSeg::Expr(x) => {
                            known = false;
                            let (t, _) = self.rvalue(x);
                            if t == Ty::Void {
                                let msg =
                                    format!("`{}` has no value to put into text", self.text(x));
                                self.err("R28.no_text_value", x.pos, msg, None);
                            }
                        }
                        StrSeg::Fmt(x, f) => {
                            known = false;
                            self.uses_2c = true;
                            let (t, _) = self.rvalue(x);
                            if t == Ty::Void {
                                let msg =
                                    format!("`{}` has no value to put into text", self.text(x));
                                self.err("R28.no_text_value", x.pos, msg, None);
                            } else if t != Ty::Err && f.conv != Conv::None && t != Ty::Num(Num::F64)
                            {
                                let spec = match f.conv {
                                    Conv::G => "g".to_string(),
                                    Conv::Fixed(n) => format!(".{n}f"),
                                    Conv::None => String::new(),
                                };
                                let msg =
                                    format!("the format {spec} needs f64, found {}", self.tn(t));
                                let hint = format!("f64({})", self.text(x));
                                self.err("R8.format_type", x.pos, msg, Some(hint));
                            } else if t != Ty::Err && f.zero && !t.is_number() {
                                let msg = format!(
                                    "the format with `0` needs a number, found {}",
                                    self.tn(t)
                                );
                                self.err(
                                    "R8.format_zero",
                                    x.pos,
                                    msg,
                                    Some("fill with spaces: drop the `0`".to_string()),
                                );
                            }
                        }
                    }
                }
                (Ty::Str, if known { Val::Known } else { Val::Runtime })
            }
            ExprKind::Ident(n) => self.lookup(n, e.pos),
            ExprKind::SelfRef => match self.cur.and_then(|id| self.fns[id].recv) {
                Some(t) => {
                    self.capture_self(Ty::Named(t));
                    (Ty::Named(t), Val::Runtime)
                }
                None => {
                    self.err(
                        "R7.self_outside_method",
                        e.pos,
                        "`self` exists only in methods",
                        None,
                    );
                    (Ty::Err, Val::Runtime)
                }
            },
            ExprKind::Paren(x) => self.expr(x),
            ExprKind::Unary(op, x) => self.unary(op, x, e.pos),
            ExprKind::Binary(op, l, r, opos) => self.binary(op, l, r, *opos),
            ExprKind::Call(f, args) => self.call(e, f, args, place),
            ExprKind::Field(base, name) => self.field(e, base, name),
            ExprKind::StructLit(name, inits) => self.struct_lit(e, name, inits),
            ExprKind::IfExpr(branches, last) => self.if_expr(e, branches, last),
            ExprKind::Named(n, x) => {
                let msg = format!("this call has no parameter `{n}`");
                self.err("R25.name_unknown", e.pos, msg, None);
                self.expr(x)
            }
            ExprKind::Mut(x) => {
                let msg = "only arguments of a call are passed with mut";
                self.err("R6.mut_not_arg", e.pos, msg, None);
                self.expr(x)
            }
            ExprKind::NoneVal => {
                self.uses_2b = true;
                let msg = "`none` needs a place of type ?T";
                self.err("R12.none_place", e.pos, msg, None);
                (Ty::Err, Val::Runtime)
            }
            ExprKind::ErrVal(_) => {
                self.uses_2b = true;
                let msg = "`err(…)` needs a place of type T!E";
                self.err("R12.err_place", e.pos, msg, None);
                (Ty::Err, Val::Runtime)
            }
            ExprKind::SomeVal(x) => self.some_val(e, x, None, None),
            ExprKind::OkVal(x) => {
                self.uses_2b = true;
                let hint = match x {
                    Some(x) => format!("return the value directly: return {}", self.text(x)),
                    None => "end the function, or write return".to_string(),
                };
                self.err(
                    "R12.ok_pattern",
                    e.pos,
                    "`ok` is only a pattern in switch",
                    Some(hint),
                );
                (Ty::Err, Val::Runtime)
            }
            ExprKind::Try(x, or) => self.try_expr(e, x, or.as_ref()),
            ExprKind::Index(base, idx, bp) => {
                let (bt, _) = self.rvalue(base);
                self.index_of(e, bt, idx, *bp)
            }
        }
    }

    fn unary(&mut self, op: &str, x: &'a Expr, pos: Pos) -> (Ty, Val) {
        if op == "!" {
            let v = self.cond(x, "!");
            return (Ty::Bool, if v == Val::Runtime { v } else { Val::Known });
        }
        let (t, v) = self.expr(x);
        if t == Ty::Err {
            return (t, Val::Runtime);
        }
        let allowed = match op {
            "-" => (t.is_int() && t.signed()) || t.is_float(),
            _ => t.is_int(),
        };
        if !allowed {
            let msg = format!("cannot apply `{op}` to {}", self.tn(t));
            let hint = match (op, t) {
                ("-", t) if t.is_int() => Some(format!(
                    "use 0 -% {} for wrapping, or a signed type",
                    self.text(x)
                )),
                (_, t) if t.is_float() => Some("use ints for bit operations".to_string()),
                _ => None,
            };
            self.err("R8.unary_type", pos, msg, hint);
            return (Ty::Err, Val::Runtime);
        }
        match (op, v) {
            ("-", Val::Int(n)) => (t, Val::Int(-n)),
            ("-", Val::Float(f)) => (t, Val::Float(-f)),
            ("~", Val::Int(n)) => {
                let (lo, hi) = t.range().unwrap_or((i64::MIN as i128, i64::MAX as i128));
                if n < lo || n > hi {
                    self.lit_fits(t, n, x);
                    return (Ty::Err, Val::Runtime);
                }
                (t, Val::Int(if lo < 0 { !n } else { hi - n }))
            }
            _ => (t, if v == Val::Runtime { v } else { Val::Known }),
        }
    }

    fn binary(&mut self, op: &'static str, l: &'a Expr, r: &'a Expr, opos: Pos) -> (Ty, Val) {
        if op == "&&" || op == "||" {
            let known = self.cond(l, op) != Val::Runtime;
            let known = self.cond(r, op) != Val::Runtime && known;
            return (Ty::Bool, if known { Val::Known } else { Val::Runtime });
        }
        if op == "??" {
            return self.coalesce(l, r, opos);
        }
        let placeless = |e: &Expr| {
            let mut e = e;
            while let ExprKind::Paren(x) = &e.kind {
                e = x;
            }
            matches!(e.kind, ExprKind::NoneVal | ExprKind::ErrVal(_))
        };
        if matches!(op, "==" | "!=") && (placeless(l) || placeless(r)) {
            // `none` and `err(e)` take their type from the other operand
            if placeless(l) && placeless(r) {
                self.expr(l);
                return (Ty::Err, Val::Runtime);
            }
            let (known, other) = if placeless(l) { (r, l) } else { (l, r) };
            let (kt, _) = self.rvalue(known);
            let (ot, _) = self.at_place(kt, other, None);
            if kt == Ty::Err || ot == Ty::Err {
                return (Ty::Err, Val::Runtime);
            }
            return (Ty::Bool, Val::Runtime);
        }
        let (lt, lv) = self.expr(l);
        let (rt, rv) = self.expr(r);
        self.arith(op, op, (l, lt, lv), (r, rt, rv), opos)
    }

    /// `l op r` for already evaluated operands; `shown` is the operator in messages
    /// (`+=` for `x += y`). A literal operand takes the type of the other (R8).
    pub(super) fn arith(
        &mut self,
        op: &str,
        shown: &str,
        (l, lt, lv): (&'a Expr, Ty, Val),
        (r, rt, rv): (&'a Expr, Ty, Val),
        opos: Pos,
    ) -> (Ty, Val) {
        if lt == Ty::Err || rt == Ty::Err {
            return (Ty::Err, Val::Runtime);
        }
        let (mut lt, mut lv, mut rt, mut rv) = (lt, lv, rt, rv);
        // literals: both literals, or one takes the type of the other
        match (lt.is_lit(), rt.is_lit()) {
            (true, true) => {
                if lt != rt {
                    // an integer literal next to a float literal is a float
                    for (t, v) in [(&mut lt, &mut lv), (&mut rt, &mut rv)] {
                        if let Val::Int(n) = *v {
                            *v = Val::Float(n as f64);
                        }
                        *t = Ty::FloatLit;
                    }
                }
            }
            (true, false) if rt.is_number() => (lt, lv) = self.adopt(rt, lt, lv, l),
            (false, true) if lt.is_number() => (rt, rv) = self.adopt(lt, rt, rv, r),
            _ => {
                (lt, lv) = self.finish(lt, lv, l, None);
                (rt, rv) = self.finish(rt, rv, r, None);
            }
        }
        if lt == Ty::Err || rt == Ty::Err {
            return (Ty::Err, Val::Runtime);
        }
        let same = lt == rt;
        let int_op = matches!(op, "+%" | "-%" | "*%" | "&" | "|" | "^" | "<<" | ">>");
        let ty = match op {
            "+" if same && lt == Ty::Str => Some(Ty::Str),
            "+" | "-" | "*" | "/" | "%" if same && lt.is_number() => Some(lt),
            _ if int_op && same && lt.is_int() => Some(lt),
            "==" | "!=" if same && lt != Ty::Void => Some(Ty::Bool),
            "<" | "<=" | ">" | ">="
                if same && (lt.is_number() || matches!(lt, Ty::Rune | Ty::Str)) =>
            {
                Some(Ty::Bool)
            }
            _ => None,
        };
        let Some(ty) = ty else {
            let msg = format!(
                "cannot apply `{shown}` to {} and {}",
                self.tn(lt),
                self.tn(rt)
            );
            let (ltext, rtext) = (self.text(l), self.text(r));
            let arithmetic = matches!(op, "+" | "-" | "*" | "/" | "%") || int_op;
            let hint = if op == "+" && (lt == Ty::Str || rt == Ty::Str) {
                if lt == Ty::Rune || rt == Ty::Rune {
                    Some("build text with interpolation: \"{s}{r}\"".to_string())
                } else {
                    Some("build text with interpolation: \"{a}{b}\"".to_string())
                }
            } else if matches!(op, "==" | "!=")
                && self.equality_hint(op, (l, lt), (r, rt)).is_some()
            {
                self.equality_hint(op, (l, lt), (r, rt))
            } else if lt.is_float() && rt.is_float() && int_op {
                Some("use ints for bit operations".to_string())
            } else if lt == Ty::Rune && rt.is_int() && arithmetic {
                Some(format!("rune(int({ltext}) {op} {rtext})"))
            } else if lt.is_number() && rt.is_number() {
                Some(format!("{ltext} {shown} {}({rtext})", self.tn(lt)))
            } else {
                None
            };
            self.err("R8.binary_types", opos, msg, hint);
            return (Ty::Err, Val::Runtime);
        };
        match (lv, rv) {
            (Val::Int(a), Val::Int(b)) if ty.is_int() => {
                return match self.fold(op, a, b, opos, (l, r), ty) {
                    Some(v) => (ty, Val::Int(v)),
                    None => (Ty::Err, Val::Runtime),
                };
            }
            (Val::Float(a), Val::Float(b)) if ty.is_float() => {
                let v = match op {
                    "+" => a + b,
                    "-" => a - b,
                    "*" => a * b,
                    "/" => a / b,
                    _ => a % b,
                };
                return (ty, Val::Float(v));
            }
            _ => {}
        }
        // a constant operand next to a runtime one is used at its place: it must fit
        for (t, v, e) in [(lt, lv, l), (rt, rv, r)] {
            if let Val::Int(n) = v {
                if t.range().is_some() && !self.lit_fits(t, n, e) {
                    return (Ty::Err, Val::Runtime);
                }
            }
        }
        let known = lv != Val::Runtime && rv != Val::Runtime;
        (ty, if known { Val::Known } else { Val::Runtime })
    }

    /// Exact arithmetic on integer constants of type `ty` (R8, R29); a certain abort
    /// is R14.
    fn fold(
        &mut self,
        op: &str,
        a: i128,
        b: i128,
        opos: Pos,
        (l, r): (&Expr, &Expr),
        ty: Ty,
    ) -> Option<i128> {
        let (lo, hi) = ty.range().unwrap_or((i64::MIN as i128, i64::MAX as i128));
        let (bits, signed) = (ty.bits(), ty.signed());
        let bitwise = matches!(op, "+%" | "-%" | "*%" | "&" | "|" | "^" | "<<" | ">>");
        if bitwise {
            for (n, e) in [(a, l), (b, r)] {
                if n < lo || n > hi {
                    self.lit_fits(ty, n, e);
                    return None;
                }
            }
        }
        // the value modulo 2^bits, as a number of the type
        let wrap = |v: i128| {
            let m = 1i128 << bits;
            let w = v.rem_euclid(m);
            if signed && w >= m / 2 {
                w - m
            } else {
                w
            }
        };
        let v = match op {
            "+" => a.checked_add(b),
            "-" => a.checked_sub(b),
            "*" => a.checked_mul(b),
            "/" | "%" if b == 0 => {
                self.err("R14.div_zero", opos, "division by zero", None);
                return None;
            }
            "/" => Some(a / b),
            "%" => Some(a % b),
            "+%" => Some(wrap(a + b)),
            "-%" => Some(wrap(a - b)),
            "*%" => Some(wrap(a.wrapping_mul(b))),
            "&" => Some(a & b),
            "|" => Some(a | b),
            "^" => Some(a ^ b),
            "<<" | ">>" if !(0..bits as i128).contains(&b) => {
                let msg = format!("shift by {b} out of range 0..{}", bits - 1);
                self.err("R14.shift_range", opos, msg, None);
                return None;
            }
            "<<" => Some(wrap(a << b)),
            ">>" => Some(a >> b),
            // comparisons of constants stay runtime values of type bool
            _ => return Some(a),
        };
        if v.is_none() {
            self.out_of_range(opos);
        }
        v
    }

    // ---- data (R5, R7) ----

    fn field(&mut self, e: &'a Expr, base: &'a Expr, name: &'a str) -> (Ty, Val) {
        if let Some(eid) = self.enum_of(base) {
            return self.variant(e, eid, name, None);
        }
        let (bt, _) = self.rvalue(base);
        if self.assigning && matches!(bt, Ty::Opt(_)) && self.map_entry(base) {
            self.map_place_error(e);
            return (Ty::Err, Val::Runtime);
        }
        match bt {
            Ty::Err => (Ty::Err, Val::Runtime),
            Ty::Named(id) => match &self.types[id as usize].kind {
                Kind::Struct(fs) => match fs.iter().find(|f| f.name == name) {
                    Some(f) => (f.ty, Val::Runtime),
                    None => {
                        let tname = self.types[id as usize].name;
                        let names: Vec<&str> = fs.iter().map(|f| f.name).collect();
                        let hint =
                            closest(name, names.into_iter()).map(|c| format!("did you mean {c}?"));
                        self.err(
                            "R4.no_field",
                            e.pos,
                            format!("`{tname}` has no field `{name}`"),
                            hint,
                        );
                        (Ty::Err, Val::Runtime)
                    }
                },
                Kind::Enum(_) => {
                    let msg = "the values of an enum are read with switch";
                    self.err("R8.enum_values_switch", e.pos, msg, None);
                    (Ty::Err, Val::Runtime)
                }
            },
            t => {
                let msg = format!("{} has no field `{name}`", self.tn(t));
                self.err("R8.no_field_type", e.pos, msg, None);
                (Ty::Err, Val::Runtime)
            }
        }
    }

    /// `E.V` or `E.V(args)`.
    fn variant(
        &mut self,
        e: &'a Expr,
        eid: u32,
        name: &str,
        args: Option<&'a [Expr]>,
    ) -> (Ty, Val) {
        let tname = self.types[eid as usize].name;
        let Kind::Enum(vs) = &self.types[eid as usize].kind else {
            return (Ty::Err, Val::Runtime);
        };
        let Some(v) = vs.iter().find(|v| v.name == name) else {
            let names: Vec<&str> = vs.iter().map(|v| v.name).collect();
            let hint =
                closest(name, names.into_iter()).map(|c| format!("did you mean {tname}.{c}?"));
            self.err(
                "R4.no_variant",
                e.pos,
                format!("`{tname}` has no variant `{name}`"),
                hint,
            );
            return (Ty::Err, Val::Runtime);
        };
        let fields = v.fields.clone();
        let list: Vec<String> = fields
            .iter()
            .map(|(t, n)| format!("{} {n}", self.tn(*t)))
            .collect();
        let full = format!("{tname}.{name}");
        match args {
            None if !fields.is_empty() => {
                let msg = format!("`{full}` needs its values: {full}({})", list.join(", "));
                self.err("R8.variant_needs_values", e.pos, msg, None);
                (Ty::Err, Val::Runtime)
            }
            None => (Ty::Named(eid), Val::Known),
            Some(_) if fields.is_empty() => {
                let msg = format!("`{full}` has no values; write {full}");
                self.err("R8.variant_no_values", e.pos, msg, None);
                (Ty::Err, Val::Runtime)
            }
            Some(args) if args.len() != fields.len() => {
                let msg = format!(
                    "`{full}` takes {} values, found {}",
                    fields.len(),
                    args.len()
                );
                self.err("R8.variant_arity", e.pos, msg, None);
                (Ty::Err, Val::Runtime)
            }
            Some(args) => {
                let mut known = true;
                for (a, (t, _)) in args.iter().zip(&fields) {
                    let (_, av) = self.at_place(*t, a, None);
                    known &= av != Val::Runtime;
                }
                (
                    Ty::Named(eid),
                    if known { Val::Known } else { Val::Runtime },
                )
            }
        }
    }

    fn struct_lit(&mut self, e: &'a Expr, name: &'a str, inits: &'a [FieldInit]) -> (Ty, Val) {
        let Some(&id) = self.type_ids.get(name) else {
            let hint =
                closest(name, self.type_ids.keys().copied()).map(|c| format!("did you mean {c}?"));
            self.err(
                "R4.unknown_type",
                e.pos,
                format!("unknown type `{name}`"),
                hint,
            );
            return (Ty::Err, Val::Runtime);
        };
        let fields: Vec<(&'a str, Ty, bool)> = match &self.types[id as usize].kind {
            Kind::Struct(fs) => fs
                .iter()
                .map(|f| (f.name, f.ty, f.default.is_some()))
                .collect(),
            Kind::Enum(_) => {
                let msg = format!("`{name}` is an enum; write {name}.Variant");
                self.err("R8.enum_as_struct", e.pos, msg, None);
                return (Ty::Err, Val::Runtime);
            }
        };
        let mut given: Vec<(&str, Pos)> = Vec::new();
        let mut known = true;
        let mut ok = true;
        for i in inits {
            let Some(&(_, fty, _)) = fields.iter().find(|f| f.0 == i.name) else {
                let names: Vec<&str> = fields.iter().map(|f| f.0).collect();
                let hint =
                    closest(&i.name, names.into_iter()).map(|c| format!("did you mean {c}?"));
                self.err(
                    "R4.no_field",
                    i.pos,
                    format!("`{name}` has no field `{}`", i.name),
                    hint,
                );
                ok = false;
                continue;
            };
            if let Some((_, p)) = given.iter().find(|(n, _)| *n == i.name) {
                let msg = format!("`{}` is given twice, first at {}", i.name, self.at(*p));
                self.err("R4.field_twice", i.pos, msg, None);
                ok = false;
                continue;
            }
            given.push((&i.name, i.pos));
            let (_, v) = self.at_place(fty, &i.value, None);
            known &= v != Val::Runtime;
        }
        let missing: Vec<(&str, Ty)> = fields
            .iter()
            .filter(|(n, _, d)| !d && !given.iter().any(|(g, _)| g == n))
            .map(|(n, t, _)| (*n, *t))
            .collect();
        if ok && !missing.is_empty() {
            let list: Vec<String> = missing.iter().map(|(n, _)| format!("`{n}`")).collect();
            let mut parts: Vec<String> = inits
                .iter()
                .map(|i| format!("{}: {}", i.name, self.text(&i.value)))
                .collect();
            parts.extend(
                missing
                    .iter()
                    .map(|(n, t)| format!("{n}: {}", self.zero(*t))),
            );
            let hint = format!("{name}{{{}}}", parts.join(", "));
            let msg = format!("`{name}` needs a value for {}", list.join(", "));
            self.err("R5.missing_fields", e.pos, msg, Some(hint));
        }
        if fields
            .iter()
            .any(|(n, _, d)| *d && !given.iter().any(|(g, _)| g == n))
        {
            known = false; // a default is computed when the literal is made
        }
        (Ty::Named(id), if known { Val::Known } else { Val::Runtime })
    }

    fn if_expr(
        &mut self,
        e: &'a Expr,
        branches: &'a [(Expr, ValueBlock)],
        last: &'a ValueBlock,
    ) -> (Ty, Val) {
        let mut ty: Option<Ty> = None;
        let all = branches
            .iter()
            .map(|(c, b)| (Some(c), b))
            .chain([(None, last)]);
        for (c, vb) in all {
            if let Some(c) = c {
                self.cond(c, "if");
            }
            self.scopes.push(Vec::new());
            for s in &vb.stmts {
                self.stmt(s);
            }
            let Some(value) = &vb.value else {
                self.scopes.pop();
                continue; // ends in return, break or continue: never arrives
            };
            let (t, _) = self.rvalue(value);
            self.scopes.pop();
            if t == Ty::Void {
                self.err(
                    "R15.branch_no_value",
                    value.pos,
                    "this branch of if gives no value",
                    None,
                );
                return (Ty::Err, Val::Runtime);
            }
            match ty {
                None => ty = Some(t),
                Some(Ty::Err) => {}
                Some(first) if t != first && t != Ty::Err => {
                    let msg = format!(
                        "the branches of if give {} and {}",
                        self.tn(first),
                        self.tn(t)
                    );
                    self.err("R8.branch_types", e.pos, msg, None);
                    return (Ty::Err, Val::Runtime);
                }
                _ => {}
            }
        }
        if ty.is_none() {
            self.err(
                "R15.no_branch_value",
                e.pos,
                "no branch of this if gives a value",
                None,
            );
        }
        (ty.unwrap_or(Ty::Err), Val::Runtime)
    }

    // ---- calls (R6, R7, R13) ----

    fn call(&mut self, e: &'a Expr, f: &'a Expr, args: &'a [Expr], place: Ty) -> (Ty, Val) {
        match &f.kind {
            ExprKind::Field(base, m) => {
                if let Some(module) = self.module_name(base) {
                    return self.module_call(e, module, m, args);
                }
                if let Some(eid) = self.enum_of(base) {
                    let r = self.variant(e, eid, m, Some(args));
                    self.last_callee = None;
                    self.last_effect = Some(false);
                    return r;
                }
                self.method_call(e, base, m, args)
            }
            ExprKind::Ident(name) => {
                let r = self.fn_call(e, f, name, args, place);
                if matches!(name.as_str(), "print" | "assert" | "expect") {
                    // an effect of its own, whatever its arguments called
                    self.last_callee = None;
                    self.last_effect = None;
                }
                r
            }
            _ => {
                let msg = format!("`{}` cannot be called", self.text(f));
                self.err("R8.not_callable", f.pos, msg, None);
                (Ty::Err, Val::Runtime)
            }
        }
    }

    fn fn_call(
        &mut self,
        e: &'a Expr,
        f: &'a Expr,
        name: &'a str,
        args: &'a [Expr],
        place: Ty,
    ) -> (Ty, Val) {
        if let Some(l) = self.local(name) {
            if let Ty::Func(fi) = l.ty {
                return self.call_param(e, f, name, fi, args);
            }
            if l.ty == Ty::Err {
                return (Ty::Err, Val::Runtime);
            }
            let msg = format!("`{name}` is {}, not a function", self.tn(l.ty));
            self.err("R8.local_not_fn", f.pos, msg, None);
            return (Ty::Err, Val::Runtime);
        }
        match name {
            "print" => {
                if args.len() != 1 {
                    let msg = format!("`print` takes 1 argument, found {}", args.len());
                    self.err("R8.arg_count", e.pos, msg, None);
                    return (Ty::Void, Val::Runtime);
                }
                for a in args {
                    let (t, _) = self.rvalue(a);
                    if t == Ty::Void {
                        let msg = format!("`{}` has no value to print", self.text(a));
                        self.err("R28.no_print_value", a.pos, msg, None);
                    }
                }
                if let Some(id) = self.cur {
                    self.facts[id].prints = true;
                }
                self.effect_in_default(e, None);
                self.last_callee = None;
                self.last_effect = Some(true);
                return (Ty::Void, Val::Runtime);
            }
            "range" => {
                let hint = "for int i in range(n) { … }".to_string();
                let msg = "`range` can only be the source of a for loop";
                self.err("R11.range_outside_for", e.pos, msg, Some(hint));
                return (Ty::Err, Val::Runtime);
            }
            "expect" => {
                if !self.in_test {
                    let cond = args.first().map_or("cond", |a| self.text(a));
                    let hint = format!("use assert({cond}) in functions");
                    let d = Diag::new(
                        "R18.expect_outside_test",
                        f.pos,
                        "`expect` is only for tests",
                    )
                    .hint(hint)
                    .fix(f.pos.offset, f.end, "assert");
                    self.diags.push(d);
                    return (Ty::Err, Val::Runtime);
                }
                if args.len() != 1 {
                    let msg = format!("`expect` takes 1 argument, found {}", args.len());
                    self.err("R8.arg_count", e.pos, msg, None);
                    return (Ty::Void, Val::Runtime);
                }
                for a in args {
                    self.cond(a, "expect");
                }
                return (Ty::Void, Val::Runtime);
            }
            "assert" => {
                self.uses_2c = true;
                if args.len() != 1 {
                    let msg = format!("`assert` takes 1 argument, found {}", args.len());
                    self.err("R8.arg_count", e.pos, msg, None);
                    return (Ty::Void, Val::Runtime);
                }
                self.cond(&args[0], "assert");
                if let Some(id) = self.cur {
                    self.facts[id].prints = true;
                }
                self.effect_in_default(e, None);
                self.last_callee = None;
                self.last_effect = Some(true);
                return (Ty::Void, Val::Runtime);
            }
            "repeat" => {
                self.uses_2c = true;
                if args.len() != 2 {
                    for a in args {
                        self.rvalue(a);
                    }
                    let msg = format!("`repeat` takes 2 arguments, found {}", args.len());
                    self.err("R8.arg_count", e.pos, msg, None);
                    return (Ty::Err, Val::Runtime);
                }
                let want = match self.peeled(place) {
                    Ty::List(i) => Some(self.lists[i as usize]),
                    _ => None,
                };
                let et = match want {
                    Some(t) => self.at_place(t, &args[0], None).0,
                    None => self.rvalue(&args[0]).0,
                };
                self.at_place(Ty::Int, &args[1], None);
                self.last_callee = None;
                self.last_effect = Some(false);
                return (self.list(want.unwrap_or(et)), Val::Runtime);
            }
            "int" | "bool" | "str" | "rune" | "i8" | "i16" | "i32" | "u8" | "u16" | "u32"
            | "u64" | "f64" => return self.convert_pure(e, name, args),
            n if self.type_ids.contains_key(n) => return self.convert_pure(e, name, args),
            _ => {}
        }
        let Some(&id) = self.funcs.get(name) else {
            if self.consts.contains_key(name) {
                let msg = format!("`{name}` is a const, not a function");
                self.err("R8.const_not_fn", f.pos, msg, None);
            } else {
                self.unknown_name(name, f.pos);
            }
            return (Ty::Err, Val::Runtime);
        };
        let ret = self.fns[id].ret;
        if !self.args(e, id, args, Vec::new()) {
            return (Ty::Err, Val::Runtime);
        }
        self.effect_in_default(e, Some(id));
        self.last_callee = Some(id);
        self.last_effect = None;
        (ret, Val::Runtime)
    }

    fn method_call(
        &mut self,
        e: &'a Expr,
        base: &'a Expr,
        m: &'a str,
        args: &'a [Expr],
    ) -> (Ty, Val) {
        let (bt, _) = self.rvalue(base);
        if let (Ty::Opt(i), true) = (bt, self.map_entry(base)) {
            // `m[k].push(x)`: an entry is changed by assigning it (R30)
            let inner = self.opts[i as usize];
            let changes = match inner {
                Ty::Named(t) => self.types[t as usize]
                    .methods
                    .get(m)
                    .is_some_and(|&id| self.fns[id].decl.mut_self.is_some()),
                _ => self.builtin_changes(inner, m),
            };
            if changes {
                self.map_place_error(base);
                for a in args {
                    self.rvalue(a);
                }
                return (Ty::Err, Val::Runtime);
            }
        }
        let tid = match bt {
            Ty::Named(tid) => tid,
            Ty::Str | Ty::Rune | Ty::List(_) | Ty::Arr(_) | Ty::Map(_) => {
                return self.builtin_method(e, base, bt, m, args)
            }
            t if t.is_number() => return self.builtin_method(e, base, bt, m, args),
            _ => {
                if bt != Ty::Err {
                    let (id, msg) = if bt == Ty::Void {
                        (
                            "R8.chain_void",
                            format!("`{m}` needs a value, but the call before it returns nothing"),
                        )
                    } else {
                        (
                            "R8.no_method",
                            format!("{} has no method `{m}`", self.tn(bt)),
                        )
                    };
                    self.err(id, e.pos, msg, None);
                }
                return (Ty::Err, Val::Runtime);
            }
        };
        let Some(&id) = self.types[tid as usize].methods.get(m) else {
            let t = &self.types[tid as usize];
            let names: Vec<&str> = t.methods.keys().copied().collect();
            let hint = closest(m, names.into_iter()).map(|c| format!("did you mean {c}?"));
            let msg = format!("`{}` has no method `{m}`", t.name);
            self.err("R4.no_method", e.pos, msg, hint);
            return (Ty::Err, Val::Runtime);
        };
        let mut changeable = Vec::new();
        let changes_self = self.fns[id].decl.mut_self.is_some();
        if changes_self {
            // the receiver is changed
            if let Some(slot) = self.root_slot(base) {
                if let Some(cur) = self.cur {
                    self.facts[cur].deps.push((id, Slot::SelfSlot, slot));
                }
            }
        }
        if changes_self {
            match self.root_of(base) {
                Some((root, false)) if is_place(base) => {
                    self.lit_changed(&root, e.pos);
                    self.call_changes
                        .entry((e.pos.offset, e.end))
                        .or_default()
                        .push((root, e.pos));
                    changeable.push(base);
                }
                Some((root, true)) if is_place(base) => {
                    let msg = format!("cannot change const `{root}`");
                    let hint = format!("`{}` changes self", self.fn_name(id));
                    self.err("R4.change_const", base.pos, msg, Some(hint));
                }
                _ => {
                    let msg = format!("`{}` changes self and needs a variable", self.fn_name(id));
                    let hint = "store the value in a variable first".to_string();
                    self.err("R7.changes_self_needs_var", e.pos, msg, Some(hint));
                }
            }
        }
        let ret = self.fns[id].ret;
        if !self.args(e, id, args, changeable) {
            return (Ty::Err, Val::Runtime);
        }
        self.effect_in_default(e, Some(id));
        self.last_callee = Some(id);
        (ret, Val::Runtime)
    }

    /// A field default calls only functions without effect (R5).
    fn effect_in_default(&mut self, e: &Expr, id: Option<usize>) {
        if !self.in_default {
            return;
        }
        let effect = match (id, &self.derived) {
            (None, _) => true, // print
            (Some(id), Some(d)) => d[id].effect,
            (Some(_), None) => false,
        };
        if effect {
            let msg = "a field default uses only constants and functions without effect";
            self.err("R5.default_effect", e.pos, msg, None);
        }
    }

    /// Arguments of a call to function `id` (R6, R8); `changeable` holds places that
    /// already go into the call changeable (a receiver).
    fn args(
        &mut self,
        e: &'a Expr,
        id: usize,
        args: &'a [Expr],
        changeable: Vec<&'a Expr>,
    ) -> bool {
        let name = self.fn_name(id);
        let ps = self.fns[id].params.iter();
        let ps: Vec<_> = ps.map(|p| (p.ty, p.is_mut, p.name.to_string())).collect();
        let decl = self.fns[id].decl;
        let Some(map) = self.named_args(e, &name, decl, args, &ps) else {
            return false;
        };
        self.args_map(e, &name, ps, Some(id), map, changeable)
    }

    /// Matches the arguments of a call to `f` to its parameters (R25): positional ones first,
    /// then `name: value` in declaration order. Returns `(parameter index, argument)`; `None`
    /// after an error.
    fn named_args(
        &mut self,
        e: &'a Expr,
        name: &str,
        f: &'a Func,
        args: &'a [Expr],
        params: &[(Ty, bool, String)],
    ) -> Option<Vec<(usize, &'a Expr)>> {
        let ps = &f.params;
        let req = ps.iter().filter(|p| p.default.is_none()).count();
        let named = |a: &Expr| matches!(a.kind, ExprKind::Named(..));
        let npos = args.iter().take_while(|a| !named(a)).count();
        let mut out: Vec<(usize, &'a Expr)> = args[..npos].iter().enumerate().collect();
        let mut bad = false;
        let mut seen: Vec<(usize, Pos)> = Vec::new();
        let mut order: Option<(Pos, String, String)> = None;
        for a in &args[npos..] {
            let ExprKind::Named(n, v) = &a.kind else {
                let msg = "a positional argument cannot follow a named one";
                self.err("R25.name_after_position", a.pos, msg, None);
                bad = true;
                continue;
            };
            let Some(k) = ps.iter().position(|p| p.name == *n) else {
                let cands = ps.iter().filter(|p| p.default.is_some());
                let cands = cands.map(|p| p.name.as_str());
                // a swapped pair of letters (`lable`) is one slip, though two edits
                let key = |s: &str| {
                    let mut v: Vec<char> = s.chars().collect();
                    v.sort();
                    v
                };
                let swapped = cands.clone().find(|c| key(c) == key(n));
                let hint = closest(n, cands)
                    .or(swapped)
                    .map(|c| format!("did you mean {c}?"));
                let msg = format!("`{name}` has no parameter `{n}`");
                self.err("R25.name_unknown", a.pos, msg, hint);
                bad = true;
                continue;
            };
            if ps[k].default.is_none() {
                let msg = format!("`{n}` has no default and is passed by position");
                let d = Diag::new("R25.name_without_default", a.pos, msg)
                    .hint(self.text(v))
                    .fix(a.pos.offset, v.pos.offset, "");
                self.diags.push(d);
                bad = true;
                continue;
            }
            if let Some(&(_, p)) = seen.iter().find(|s| s.0 == k) {
                let msg = format!("`{n}` is given twice, first at {}", self.at(p));
                self.err("R25.name_twice", a.pos, msg, None);
                bad = true;
                continue;
            }
            if order.is_none() {
                if let Some(&(pk, _)) = seen.iter().find(|s| s.0 > k) {
                    order = Some((a.pos, n.clone(), ps[pk].name.clone()));
                }
            }
            seen.push((k, a.pos));
            out.push((k, v));
        }
        if let Some((pos, n, later)) = order {
            let msg = format!("`{n}` comes before `{later}` in the declaration");
            let mut sorted: Vec<&Expr> = args[..npos].iter().collect();
            let mut by_decl: Vec<(usize, &Expr)> = args[npos..]
                .iter()
                .filter_map(|a| match &a.kind {
                    ExprKind::Named(n, _) => ps.iter().position(|p| p.name == *n).map(|k| (k, a)),
                    _ => None,
                })
                .collect();
            by_decl.sort_by_key(|x| x.0);
            sorted.extend(by_decl.into_iter().map(|x| x.1));
            let list: Vec<&str> = sorted.iter().map(|a| self.text(a)).collect();
            let list = list.join(", ");
            let mut d = Diag::new("R25.name_order", pos, msg).hint(list.clone());
            if !bad {
                let last = args.last().map_or(0, |a| a.end);
                d = d.fix(args[0].pos.offset, last, list);
            }
            self.diags.push(d);
            bad = true;
        }
        if bad {
            return None;
        }
        if npos > ps.len() || npos < req {
            let s = if req == 1 { "" } else { "s" };
            let msg = format!("`{name}` takes {req} argument{s}, found {}", args.len());
            let hint = self.header_block_hint(e, params, args.len());
            self.err("R8.arg_count", e.pos, msg, hint);
            return None;
        }
        if npos > req {
            for (k, a) in out.iter().skip(req) {
                let n = &ps[*k].name;
                let msg = format!("`{n}` has a default and is passed by name");
                let d = Diag::new("R25.positional_default", a.pos, msg)
                    .hint(format!("{n}: {}", self.text(a)))
                    .fix(a.pos.offset, a.pos.offset, format!("{n}: "));
                self.diags.push(d);
            }
            return None;
        }
        Some(out)
    }

    /// Arguments for parameters `(type, mut, name)`; `callee` is the function, or `None` for a
    /// function-typed parameter, whose `mut` counts as a change (R26).
    pub(super) fn args_to(
        &mut self,
        e: &'a Expr,
        name: &str,
        params: Vec<(Ty, bool, String)>,
        callee: Option<usize>,
        args: &'a [Expr],
        changeable: Vec<&'a Expr>,
    ) -> bool {
        if let Some(a) = args.iter().find(|a| matches!(a.kind, ExprKind::Named(..))) {
            let msg = "a function value has no parameter names";
            self.err("R25.name_on_value", a.pos, msg, None);
            return false;
        }
        let n = params.len();
        if args.len() != n {
            let s = if n == 1 { "" } else { "s" };
            let msg = format!("`{name}` takes {n} argument{s}, found {}", args.len());
            let hint = self.header_block_hint(e, &params, args.len());
            self.err("R8.arg_count", e.pos, msg, hint);
            return false;
        }
        let map = args.iter().enumerate().collect();
        self.args_map(e, name, params, callee, map, changeable)
    }

    fn args_map(
        &mut self,
        e: &'a Expr,
        name: &str,
        params: Vec<(Ty, bool, String)>,
        callee: Option<usize>,
        args: Vec<(usize, &'a Expr)>,
        mut changeable: Vec<&'a Expr>,
    ) -> bool {
        if let (Some(cur), Some(id)) = (self.cur, callee) {
            self.facts[cur].calls.push(id);
        }
        for (j, a) in args {
            let (pt, pmut) = (params[j].0, params[j].1);
            let pname = params[j].2.as_str();
            match (&a.kind, pmut) {
                (ExprKind::Mut(inner), false) => {
                    let msg = format!("`{name}` takes `{pname}` without mut");
                    let hint = format!(
                        "{}{}{}",
                        self.span(e.pos.offset, a.pos.offset),
                        self.text(inner),
                        self.span(a.end, e.end)
                    );
                    let d = Diag::new("R6.mut_extra", a.pos, msg).hint(hint).fix(
                        a.pos.offset,
                        inner.pos.offset,
                        "",
                    );
                    self.diags.push(d);
                    self.at_place(pt, inner, None);
                }
                (ExprKind::Mut(inner), true) => {
                    if !is_place(inner) {
                        let msg = "only a variable, field or element can be passed as mut";
                        self.err("R6.mut_not_place", a.pos, msg, None);
                        continue;
                    }
                    if let Some((root, true)) = self.root_of(inner) {
                        let msg = format!("cannot pass const `{root}` as mut");
                        self.err("R4.pass_const_mut", inner.pos, msg, None);
                        continue;
                    }
                    let (t, _) = self.expr(inner);
                    if self.map_entry(inner) {
                        self.map_place_error(inner);
                        continue;
                    }
                    self.expect(pt, t, inner);
                    let changes = match callee {
                        Some(id) => {
                            if let (Some(cur), Some(slot)) = (self.cur, self.root_slot(inner)) {
                                self.facts[cur].deps.push((id, Slot::Param(j), slot));
                            }
                            self.derived.as_ref().is_some_and(|d| d[id].changes[j])
                        }
                        None => {
                            self.changed(inner);
                            true
                        }
                    };
                    if changes {
                        if let Some((root, _)) = self.root_of(inner) {
                            self.lit_changed(&root, a.pos);
                            self.call_changes
                                .entry((e.pos.offset, e.end))
                                .or_default()
                                .push((root, a.pos));
                        }
                    }
                    if let Some(other) = changeable.iter().find(|o| overlap(o, inner)) {
                        let root = self.root_of(other).map(|r| r.0).unwrap_or_default();
                        let msg = format!("`{root}` goes into this call changeable twice");
                        let hint = match (&other.kind, &inner.kind) {
                            (ExprKind::Index(b, ..), ExprKind::Index(..)) => {
                                format!("to swap two elements use {}.swap(i, j)", self.text(b))
                            }
                            _ => "pass it changeable only once".to_string(),
                        };
                        self.err("R6.mut_twice", a.pos, msg, Some(hint));
                    } else {
                        changeable.push(inner);
                    }
                }
                (_, true) => {
                    let what = if pname.starts_with("argument ") {
                        pname.to_string()
                    } else {
                        format!("`{pname}`")
                    };
                    let msg = format!("`{name}` changes its {what}; pass it as mut");
                    let hint = format!(
                        "{}mut {}",
                        self.span(e.pos.offset, a.pos.offset),
                        self.span(a.pos.offset, e.end)
                    );
                    let d = Diag::new("R6.mut_missing", a.pos, msg).hint(hint).fix(
                        a.pos.offset,
                        a.pos.offset,
                        "mut ",
                    );
                    self.diags.push(d);
                    self.at_place(pt, a, None);
                }
                (_, false) => {
                    self.arg_value(e, name, pt, a);
                }
            }
        }
        true
    }
}

/// Part 2b: results, optionals, `try`, `??` and tests (R12, R13, R18, R28).
impl<'a> Checker<'a> {
    /// A compile-time value of a compound type is only "known", never an `int`.
    fn mask(&self, ty: Ty, v: Val) -> Val {
        match (ty, v) {
            (Ty::Opt(_) | Ty::Res(_), Val::Int(_) | Val::Float(_)) => Val::Known,
            _ => v,
        }
    }

    /// `e` where a value of type `want` is expected (R8, R31). `none`, `err(…)` and
    /// `some(…)` take their type from this place (R12). `decl` is the name of the
    /// variable being declared, for the hint. Returns the type found and its value.
    fn at_place(&mut self, want: Ty, e: &'a Expr, decl: Option<&str>) -> (Ty, Val) {
        if want == Ty::Err {
            // the type of the place was reported; an empty literal needs no second message
            let empty = match &e.kind {
                ExprKind::ListLit(v) => v.is_empty(),
                ExprKind::MapLit(v) => v.is_empty(),
                _ => false,
            };
            if empty {
                return (Ty::Err, Val::Runtime);
            }
        }
        let r = match &e.kind {
            ExprKind::Paren(x) if !matches!(x.kind, ExprKind::Missing) => {
                self.at_place(want, x, decl)
            }
            ExprKind::NoneVal => {
                self.uses_2b = true;
                match want {
                    Ty::Opt(_) | Ty::Err => (want, Val::Known),
                    // `?T!E`: `none` is a `?T` that is not an error
                    Ty::Res(i) if matches!(self.ress[i as usize].0, Ty::Opt(_)) => {
                        (want, Val::Known)
                    }
                    _ => {
                        let hint = decl
                            .filter(|_| !matches!(want, Ty::Res(_)))
                            .map(|n| format!("declare it as ?{} {n}", self.tn(want)));
                        self.wrong_place(want, "none", e, hint);
                        (Ty::Err, Val::Runtime)
                    }
                }
            }
            ExprKind::ErrVal(x) => {
                self.uses_2b = true;
                match want {
                    Ty::Res(i) => {
                        let et = self.ress[i as usize].1;
                        let (_, v) = self.at_place(et, x, None);
                        (want, if v == Val::Runtime { v } else { Val::Known })
                    }
                    Ty::Err => {
                        self.rvalue(x);
                        (Ty::Err, Val::Runtime)
                    }
                    _ => {
                        let (et, _) = self.rvalue(x);
                        let hint = decl
                            .filter(|_| et != Ty::Err && !matches!(want, Ty::Opt(_)))
                            .map(|n| {
                                format!("declare it as {}!{} {n}", self.tn(want), self.tn(et))
                            });
                        self.wrong_place(want, "err(…)", e, hint);
                        (Ty::Err, Val::Runtime)
                    }
                }
            }
            ExprKind::SomeVal(x) => {
                let (t, v) = self.some_val(e, x, Some(want), None);
                if t != Ty::Err && !self.fits_place(want, t) && want != Ty::Err {
                    let hint = decl.map(|n| format!("declare it as {} {n}", self.tn(t)));
                    self.wrong_place(want, &self.tn(t), e, hint);
                    (Ty::Err, Val::Runtime)
                } else {
                    (t, v)
                }
            }
            _ => {
                let target = self.peeled(want);
                let lit_float = target == Ty::Num(Num::F64) && lit_only(e);
                let saved = std::mem::replace(&mut self.lit_float, lit_float);
                self.place_ty = target;
                let (t, v) = self.expr(e);
                self.lit_float = saved;
                self.settle(want, t, v, e)
            }
        };
        self.exprs.insert((e.pos.offset, e.end), r.0);
        r
    }

    fn wrong_place(&mut self, want: Ty, found: &str, e: &Expr, hint: Option<String>) {
        if want != Ty::Err {
            let msg = format!("expected {}, found {found}", self.tn(want));
            self.err("R8.type_mismatch", e.pos, msg, hint);
        }
    }

    /// `some(x)`; at a place `?T` the value `x` has that place's `T`.
    fn some_val(
        &mut self,
        e: &'a Expr,
        x: &'a Expr,
        want: Option<Ty>,
        _decl: Option<&str>,
    ) -> (Ty, Val) {
        self.uses_2b = true;
        let want_opt = match want {
            Some(Ty::Res(i)) => Some(self.ress[i as usize].0),
            w => w,
        };
        let inner_want = match want_opt {
            Some(Ty::Opt(i)) => Some(self.opts[i as usize]),
            _ => None,
        };
        let (xt, xv) = match (&x.kind, inner_want) {
            (ExprKind::NoneVal | ExprKind::ErrVal(_) | ExprKind::SomeVal(_), Some(w)) => {
                self.at_place(w, x, None)
            }
            _ => {
                let (t, v) = self.expr(x);
                self.finish(t, v, x, inner_want)
            }
        };
        if matches!(xt, Ty::Opt(_)) {
            let msg = format!("`some` needs a plain value, found {}", self.tn(xt));
            let hint = format!("use {} directly", self.text(x));
            self.err("R12.some_plain", e.pos, msg, Some(hint));
            return (Ty::Err, Val::Runtime);
        }
        let known = if xv == Val::Runtime { xv } else { Val::Known };
        match inner_want {
            Some(w) => {
                self.expect(w, xt, x);
                (want.unwrap_or(Ty::Err), known)
            }
            None => (self.opt(xt), known),
        }
    }

    /// `a ?? b` (R12).
    fn coalesce(&mut self, l: &'a Expr, r: &'a Expr, opos: Pos) -> (Ty, Val) {
        self.uses_2b = true;
        let (lt, _) = self.rvalue(l);
        let inner = match self.inner(lt) {
            Some(t) if t != Ty::Void => t,
            _ if lt == Ty::Err => {
                self.rvalue(r);
                return (Ty::Err, Val::Runtime);
            }
            _ => {
                let msg = format!("`??` needs a ?T or T!E on the left, found {}", self.tn(lt));
                let hint = "remove the ?? and its default".to_string();
                self.err("R12.coalesce_left", opos, msg, Some(hint));
                self.rvalue(r);
                return (Ty::Err, Val::Runtime);
            }
        };
        self.at_place(inner, r, None);
        (inner, Val::Runtime)
    }

    fn equality_hint(
        &self,
        op: &str,
        (l, lt): (&Expr, Ty),
        (r, rt): (&Expr, Ty),
    ) -> Option<String> {
        match (lt, rt) {
            (Ty::Opt(_), t) if self.inner(lt) == Some(t) => {
                Some(format!("{} {op} some({})", self.text(l), self.text(r)))
            }
            (t, Ty::Opt(_)) if self.inner(rt) == Some(t) => {
                Some(format!("some({}) {op} {}", self.text(l), self.text(r)))
            }
            (Ty::Res(_), t) if self.inner(lt) == Some(t) => {
                Some("use switch to look at the value".into())
            }
            (t, Ty::Res(_)) if self.inner(rt) == Some(t) => {
                Some("use switch to look at the value".into())
            }
            _ => None,
        }
    }

    /// `try x` and `try x or e` (R12, R18).
    fn try_expr(&mut self, e: &'a Expr, x: &'a Expr, or: Option<&'a TryOr>) -> (Ty, Val) {
        self.uses_2b = true;
        let (xt, _) = self.rvalue(x);
        let callee = self.last_callee;
        self.try_with(e, xt, callee, or)
    }

    /// The `or` part of `try x or …` for a function whose error type is `f` (R12): a
    /// value of type `f`, `err(E e): value` binding the cause `e` of a `T!E`, or the
    /// short form `F.V` of a variant that holds exactly the cause.
    fn try_or(&mut self, o: &'a TryOr, xt: Ty, f: Ty) {
        let cause = match xt {
            Ty::Res(i) => Some(self.ress[i as usize].1),
            _ => None,
        };
        if let Some(p) = &o.bind {
            let bt = self.ty(&p.ty, false);
            let mut have = Ty::Err;
            match cause {
                None => {
                    let msg = format!("`try` on {} has no error to bind", self.tn(xt));
                    self.err(
                        "R12.try_no_error_bind",
                        p.ty.pos,
                        msg,
                        Some("remove err(…):".into()),
                    );
                }
                Some(c) if bt == c || bt == Ty::Err || c == Ty::Err => have = c,
                Some(c) => {
                    let tn = self.tn(c);
                    let art = if tn.starts_with(['a', 'e', 'i', 'o', 'u', 'A', 'E', 'I', 'O', 'U'])
                    {
                        "an"
                    } else {
                        "a"
                    };
                    let hint = format!("or err({tn} {}): …", p.name);
                    let msg = format!("`err` holds {art} {tn}");
                    self.err("R8.bind_type", p.ty.pos, msg, Some(hint));
                }
            }
            self.scopes.push(Vec::new());
            self.declare(&p.name, p.pos, have, false, Val::Runtime, None);
            self.at_place(f, &o.alt, None);
            self.scopes.pop();
            return;
        }
        if let (Some(c), Some((eid, name))) = (cause, self.short_variant(&o.alt)) {
            if Ty::Named(eid) == f {
                let Kind::Enum(vs) = &self.types[eid as usize].kind else {
                    return;
                };
                if let Some(v) = vs
                    .iter()
                    .find(|v| v.name == name)
                    .filter(|v| !v.fields.is_empty())
                {
                    let full = format!("{}.{name}", self.types[eid as usize].name);
                    let (id, msg) = match v.fields[..] {
                        [(t, _)] if t == c => return,
                        [(t, _)] => (
                            "R8.alt_holds",
                            format!(
                                "`{full}` holds {}, but the cause is {}",
                                self.tn(t),
                                self.tn(c)
                            ),
                        ),
                        _ => (
                            "R8.alt_values",
                            format!(
                                "`{full}` has {} values, but the cause is one",
                                v.fields.len()
                            ),
                        ),
                    };
                    let hint = format!("write it out: or err({} e): {full}(…)", self.tn(c));
                    self.err(id, o.alt.pos, msg, Some(hint));
                    return;
                }
            }
        }
        self.at_place(f, &o.alt, None);
    }

    /// `E.V` written without parentheses: the enum and the variant name.
    fn short_variant(&self, alt: &'a Expr) -> Option<(u32, &'a str)> {
        match &alt.kind {
            ExprKind::Field(base, name) => Some((self.enum_of(base)?, name.as_str())),
            _ => None,
        }
    }

    /// The rules of `try` for a value of type `xt`: the expression `try x`, or the
    /// elements of `for T x in try xs` (`e` is the `try` expression, for positions).
    fn try_with(
        &mut self,
        e: &'a Expr,
        xt: Ty,
        callee: Option<usize>,
        or: Option<&'a TryOr>,
    ) -> (Ty, Val) {
        let Some(value) = self.inner(xt) else {
            if xt != Ty::Err {
                let msg = format!(
                    "`try` needs a result of type T!E or ?T, found {}",
                    self.tn(xt)
                );
                self.err("R12.try_operand", e.pos, msg, Some("remove try".into()));
            }
            return (Ty::Err, Val::Runtime);
        };
        let ret = if self.in_test && self.frames.is_empty() {
            None
        } else {
            let cur = self.frames.last().map(|f| f.ret);
            match cur.or_else(|| self.cur.map(|id| self.fns[id].ret)) {
                Some(Ty::Err) => return (Ty::Err, Val::Runtime),
                Some(r) => Some(r),
                None => {
                    let msg = "`try` needs a function that returns T!E or ?T";
                    self.err("R12.try_needs_result_fn", e.pos, msg, None);
                    return (Ty::Err, Val::Runtime);
                }
            }
        };
        let Some(ret) = ret else {
            // in a test any failure ends the test; there is no error type to translate to
            if let Some(o) = or {
                let msg = "`or` needs a function that returns U!F";
                self.err(
                    "R12.or_needs_result_fn",
                    o.pos,
                    msg,
                    Some("remove the or part".into()),
                );
                return (Ty::Err, Val::Runtime);
            }
            return (value, Val::Runtime);
        };
        let from = match xt {
            Ty::Res(i) => self.tn(self.ress[i as usize].1),
            _ => "none".to_string(),
        };
        match ret {
            Ty::Res(i) => {
                let f = self.ress[i as usize].1;
                match or {
                    Some(o) => {
                        self.try_or(o, xt, f);
                        self.last_callee = callee;
                    }
                    None if matches!(xt, Ty::Res(j) if self.ress[j as usize].1 == f) => {}
                    None => {
                        let msg = format!(
                            "`try` gives {from}, but the function returns {}",
                            self.tn(ret)
                        );
                        let hint = match f {
                            Ty::Named(id)
                                if self.enum_id(self.types[id as usize].name).is_some() =>
                            {
                                format!("translate it with try … or {}.V(…)", self.tn(f))
                            }
                            _ => format!("translate it with try … or <a {}>", self.tn(f)),
                        };
                        self.err("R12.try_error_mismatch", e.pos, msg, Some(hint));
                        return (Ty::Err, Val::Runtime);
                    }
                }
            }
            Ty::Opt(i) => {
                if let Some(o) = or {
                    let msg = "`or` needs a function that returns U!F";
                    let hint = format!(
                        "remove the or part, or return {}!F",
                        self.tn(self.opts[i as usize])
                    );
                    self.err("R12.or_needs_result_fn", o.pos, msg, Some(hint));
                    return (Ty::Err, Val::Runtime);
                }
                if matches!(xt, Ty::Res(_)) {
                    let msg = format!(
                        "`try` gives {from}, but the function returns {}",
                        self.tn(ret)
                    );
                    let hint = "handle the error with ?? or switch".to_string();
                    self.err("R12.try_error_mismatch", e.pos, msg, Some(hint));
                    return (Ty::Err, Val::Runtime);
                }
            }
            _ => {
                let want = match xt {
                    Ty::Res(i) => format!("{}!{}", self.tn(ret), self.tn(self.ress[i as usize].1)),
                    _ => format!("?{}", self.tn(ret)),
                };
                let msg = "`try` needs a function that returns T!E or ?T";
                let hint = if ret == Ty::Void && matches!(xt, Ty::Opt(_)) {
                    "use ?? or switch".to_string() // ?void does not exist
                } else {
                    format!("change the return type to {want}, or use ?? or switch")
                };
                self.err("R12.try_needs_result_fn", e.pos, msg, Some(hint));
                return (Ty::Err, Val::Runtime);
            }
        }
        (value, Val::Runtime)
    }

    /// A `test` block (R18).
    fn test(&mut self, t: &'a TestDecl) {
        self.uses_2b = true;
        match self.tests.iter().find(|(n, _)| *n == t.name) {
            Some(&(_, first)) => {
                let msg = format!("test \"{}\" is declared twice", t.name);
                let hint = format!("first declared at {}", self.at(first));
                self.err("R18.test_twice", t.name_pos, msg, Some(hint));
            }
            None => self.tests.push((&t.name, t.name_pos)),
        }
        self.in_test = true;
        self.fn_locals.clear();
        let before = self.diags.len();
        self.block(&t.body);
        if self.derived.is_some() && self.diags.len() == before {
            self.moves
                .extend(live::moves(&t.body, &self.call_changes, &[]));
            for (place, pos) in live::lost(&t.body, &self.call_changes, &[]) {
                if self
                    .fn_locals
                    .contains(place.split('.').next().unwrap_or(""))
                {
                    let msg = format!("`{place}` is changed but never read afterwards");
                    let hint = self
                        .lost_hints
                        .get(&pos.offset)
                        .cloned()
                        .unwrap_or_else(|| "use the value, or remove the change".to_string());
                    self.err("R6.dead_change", pos, msg, Some(hint));
                }
            }
        }
        self.in_test = false;
    }
}

fn is_place(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Ident(_) | ExprKind::SelfRef => true,
        ExprKind::Field(b, _) | ExprKind::Index(b, _, _) => is_place(b),
        _ => false,
    }
}

/// The path of a place: root, fields, and indexes (`#n` for a constant index, `#?` for
/// one only known at run time).
fn path(e: &Expr) -> Vec<String> {
    match &e.kind {
        ExprKind::Ident(n) => vec![n.clone()],
        ExprKind::SelfRef => vec!["self".into()],
        ExprKind::Field(b, f) => {
            let mut p = path(b);
            p.push(f.clone());
            p
        }
        ExprKind::Index(b, i, _) => {
            let mut p = path(b);
            p.push(match &i.kind {
                ExprKind::Int(n) => format!("#{n}"),
                _ => "#?".into(),
            });
            p
        }
        _ => Vec::new(),
    }
}

/// Two places overlap when one path starts with the other (R6); `xs[0]` and `xs[1]` never
/// do, and two indexes of which one is only known at run time are checked there.
fn overlap(a: &Expr, b: &Expr) -> bool {
    let (pa, pb) = (path(a), path(b));
    let n = pa.len().min(pb.len());
    n > 0
        && pa[..n]
            .iter()
            .zip(&pb[..n])
            .all(|(x, y)| x == y && x != "#?")
}

/// Does every path through the block end in `return`? `complete` holds the
/// switches that cover every value (R8, R10).
fn terminates(b: &Block, complete: &HashSet<u32>) -> bool {
    match b.stmts.last() {
        Some(Stmt::Return(..)) => true,
        Some(Stmt::If {
            branches,
            otherwise: Some(o),
            ..
        }) => branches.iter().all(|(_, b)| terminates(b, complete)) && terminates(o, complete),
        Some(Stmt::Switch {
            cases,
            default,
            pos,
            ..
        }) => {
            complete.contains(&pos.offset)
                && cases.iter().all(|c| terminates(&c.body, complete))
                && default
                    .as_ref()
                    .is_none_or(|(d, _)| terminates(d, complete))
        }
        Some(Stmt::While { cond, body, .. }) => {
            matches!(cond.kind, ExprKind::Bool(true)) && !breaks(body)
        }
        _ => false,
    }
}

/// Does a `break` leave this loop body (not one of an inner loop)?
fn breaks(b: &Block) -> bool {
    b.stmts.iter().any(|s| match s {
        Stmt::Break(_) => true,
        Stmt::If {
            branches,
            otherwise,
            ..
        } => branches.iter().any(|(_, b)| breaks(b)) || otherwise.as_ref().is_some_and(breaks),
        Stmt::Switch { cases, default, .. } => {
            cases.iter().any(|c| breaks(&c.body))
                || default.as_ref().is_some_and(|(d, _)| breaks(d))
        }
        _ => false,
    })
}

/// The candidate with the smallest edit distance, if it is close enough.
fn closest<'b>(name: &str, candidates: impl Iterator<Item = &'b str>) -> Option<&'b str> {
    let limit = (name.chars().count() / 3).max(1);
    candidates
        .filter(|c| *c != name)
        .map(|c| (distance(name, c), c))
        .filter(|(d, _)| *d <= limit)
        .min()
        .map(|(_, c)| c)
}

fn distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut prev = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cur = row[j + 1];
            row[j + 1] = (prev + usize::from(ca != *cb)).min(row[j] + 1).min(cur + 1);
            prev = cur;
        }
    }
    row[b.len()]
}

/// A number expression made of literals only: `7 / 2`, `-(1 + 2)`. At the place of a
/// `f64` it is computed in `f64` (R8).
fn lit_only(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Int(_) | ExprKind::Float(_) => true,
        ExprKind::Paren(x) | ExprKind::Unary("-", x) => lit_only(x),
        ExprKind::Binary("+" | "-" | "*" | "/" | "%", l, r, _) => lit_only(l) && lit_only(r),
        _ => false,
    }
}

/// A call over several lines as one line: each line break and the indentation after it is
/// one space (R13).
fn flat(s: &str) -> String {
    s.split('\n')
        .enumerate()
        .map(|(i, l)| if i == 0 { l } else { l.trim_start() })
        .collect::<Vec<_>>()
        .join(" ")
}
