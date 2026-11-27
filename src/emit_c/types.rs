//! Types of the C program: definitions, descriptors for the runtime (`dif_ty`), ownership
//! operations (copy, release) and the helpers of struct, enum, `?T` and `T!E` values
//! (equality, text form, hash). The runtime owns everything that is generic.

use std::collections::HashSet;
use std::fmt::Write;

use super::Emitter;
use crate::checker::{Num, Ty, TypeBody};

/// A C identifier from a type name (`io.IoErr` is `io_IoErr`).
pub(super) fn cid(name: &str) -> String {
    name.replace('.', "_")
}

impl<'a> Emitter<'a> {
    // ---- names ----

    pub(super) fn c_ty(&self, t: Ty) -> String {
        match t {
            Ty::Int | Ty::IntLit => "int64_t".into(),
            Ty::Bool => "bool".into(),
            Ty::Str => "dif_str".into(),
            Ty::Rune => "uint32_t".into(),
            Ty::Num(n) => match n {
                Num::I8 => "int8_t",
                Num::I16 => "int16_t",
                Num::I32 => "int32_t",
                Num::U8 => "uint8_t",
                Num::U16 => "uint16_t",
                Num::U32 => "uint32_t",
                Num::U64 => "uint64_t",
                Num::F64 => "double",
            }
            .into(),
            Ty::FloatLit => "double".into(),
            Ty::List(_) => "dif_list *".into(),
            Ty::Map(_) => "dif_map *".into(),
            Ty::Arr(i) => format!("dif_a{i}"),
            Ty::Named(id) => format!("d_{}", cid(&self.info.types[id as usize].name)),
            Ty::Opt(i) => format!("dif_o{i}"),
            Ty::Res(i) => format!("dif_r{i}"),
            Ty::Func(i) => format!("dfn_{i}"),
            Ty::Void | Ty::Err => "void".into(),
        }
    }

    /// Name part of the helpers (`dif_eq_…`, `dif_put_…`, `dif_print_…`) of a type.
    pub(super) fn sfx(&self, t: Ty) -> String {
        match t {
            Ty::Named(id) => cid(&self.info.types[id as usize].name),
            Ty::Opt(i) => format!("__o{i}"),
            Ty::Res(i) => format!("__r{i}"),
            Ty::List(i) => format!("__l{i}"),
            Ty::Arr(i) => format!("__a{i}"),
            Ty::Map(i) => format!("__m{i}"),
            _ => String::new(),
        }
    }

    /// The descriptor (`dif_ty`) of a type.
    pub(super) fn dty(&self, t: Ty) -> String {
        match t {
            Ty::Int | Ty::IntLit => "dif_ty_int".into(),
            Ty::Bool => "dif_ty_bool".into(),
            Ty::Str => "dif_ty_str".into(),
            Ty::Rune => "dif_ty_rune".into(),
            Ty::Num(n) => format!("dif_ty_{}", n.name()),
            Ty::FloatLit => "dif_ty_f64".into(),
            t => format!("dty_{}", self.sfx(t)),
        }
    }

    pub(super) fn elem_of(&self, t: Ty) -> Ty {
        match t {
            Ty::List(i) => self.info.lists[i as usize],
            Ty::Arr(i) => self.info.arrs[i as usize].0,
            _ => Ty::Err,
        }
    }

    /// Does the type have helpers generated for it (struct, enum, `?T`, `T!E`)?
    pub(super) fn composite(t: Ty) -> bool {
        matches!(t, Ty::Named(_) | Ty::Opt(_) | Ty::Res(_))
    }

    /// Does a value of the type hold counted memory (text, list, map)?
    pub(super) fn managed(&self, t: Ty) -> bool {
        match t {
            Ty::Str | Ty::List(_) | Ty::Map(_) => true,
            Ty::Opt(i) => self.managed(self.info.opts[i as usize]),
            Ty::Res(i) => {
                let (v, e) = self.info.ress[i as usize];
                self.managed(v) || self.managed(e)
            }
            Ty::Arr(i) => self.managed(self.info.arrs[i as usize].0),
            Ty::Named(id) => match &self.info.types[id as usize].body {
                TypeBody::Struct(fs) => fs.iter().any(|f| self.managed(f.1)),
                TypeBody::Enum(vs) => vs
                    .iter()
                    .any(|v| v.1.iter().any(|f| self.boxed(id, f.0) || self.managed(f.0))),
            },
            _ => false,
        }
    }

    /// Does a value of type `t` hold the enum `target` by value (struct field, array,
    /// `?T`, `T!E`, or the payload of another enum)?
    fn holds(&self, t: Ty, target: u32, seen: &mut HashSet<u32>) -> bool {
        match t {
            Ty::Named(id) if id == target => true,
            Ty::Named(id) => {
                seen.insert(id)
                    && match &self.info.types[id as usize].body {
                        TypeBody::Struct(fs) => fs.iter().any(|f| self.holds(f.1, target, seen)),
                        TypeBody::Enum(vs) => vs
                            .iter()
                            .any(|v| v.1.iter().any(|f| self.holds(f.0, target, seen))),
                    }
            }
            Ty::Opt(i) => self.holds(self.info.opts[i as usize], target, seen),
            Ty::Res(i) => {
                let (v, e) = self.info.ress[i as usize];
                self.holds(v, target, seen) || self.holds(e, target, seen)
            }
            Ty::Arr(i) => self.holds(self.info.arrs[i as usize].0, target, seen),
            _ => false,
        }
    }

    /// Is a payload field of type `t` in enum `eid` kept in a counted block (R5)? It is
    /// when the field leads back to the enum; then the field is a `T *` into the block.
    pub(super) fn boxed(&self, eid: u32, t: Ty) -> bool {
        self.holds(t, eid, &mut HashSet::new())
    }

    /// The C lvalue of payload field `j` of variant `k`: `path` is `<value>.u.v<k>.f<j>`.
    pub(super) fn fld(&self, eid: u32, k: usize, j: usize, path: String) -> String {
        match &self.info.types[eid as usize].body {
            TypeBody::Enum(vs) if self.boxed(eid, vs[k].1[j].0) => format!("(*{path})"),
            _ => path,
        }
    }

    // ---- ownership ----

    /// A second owner of the value `v` (borrowed or owned): a counted copy.
    pub(super) fn copy(&self, t: Ty, v: &str) -> String {
        match t {
            Ty::Str => format!("dif_str_inc({v})"),
            Ty::List(_) => format!("dif_list_inc((dif_list *)({v}))"),
            Ty::Map(_) => format!("dif_map_inc((dif_map *)({v}))"),
            t if self.managed(t) => format!(
                "({{ {} c_ = ({v}); dif_inc(&{}, &c_); c_; }})",
                self.c_ty(t),
                self.dty(t)
            ),
            _ => v.to_string(),
        }
    }

    /// An expression that releases the value in the place `p` (an lvalue).
    pub(super) fn drop_e(&self, t: Ty, p: &str) -> String {
        match t {
            Ty::Str => format!("dif_str_dec({p})"),
            Ty::List(i) => format!(
                "dif_list_dec(&{}, {p})",
                self.dty(self.info.lists[i as usize])
            ),
            Ty::Map(i) => {
                let (k, v) = self.info.maps[i as usize];
                format!("dif_map_dec(&{}, &{}, {p})", self.dty(k), self.dty(v))
            }
            t => format!("dif_dec(&{}, &({p}))", self.dty(t)),
        }
    }

    /// The value of a released place: nothing owned, safe to release again.
    pub(super) fn zero(&self, t: Ty) -> String {
        match t {
            Ty::Str => "DIF_EMPTY".into(),
            Ty::List(_) | Ty::Map(_) => "NULL".into(),
            t => format!("(({}){{0}})", self.c_ty(t)),
        }
    }

    /// Statements: release the place `p` and leave it empty.
    pub(super) fn rel(&self, t: Ty, p: &str) -> String {
        format!("{}; {p} = {};", self.drop_e(t, p), self.zero(t))
    }

    /// An expression that takes the value out of the temporary `t` and leaves it empty.
    pub(super) fn take_temp(name: &str) -> String {
        format!("DIF_MOVE({name})")
    }

    // ---- definitions ----

    /// Every type of the module, after the types it contains (a value has a fixed size, R5).
    pub(super) fn type_defs(&self) -> String {
        let mut done = HashSet::new();
        let mut c = String::new();
        // a block pointer needs only the name of its type
        for t in self.universe() {
            if matches!(t, Ty::Named(_) | Ty::Opt(_) | Ty::Res(_) | Ty::Arr(_)) {
                let n = self.c_ty(t);
                let _ = writeln!(c, "typedef struct {n} {n};");
            }
        }
        for t in self.universe() {
            self.type_def(t, &mut done, &mut c);
        }
        c.push_str(&self.fn_type_defs());
        c
    }

    /// All types that can have a definition or descriptor.
    pub(super) fn universe(&self) -> Vec<Ty> {
        let n = self.info;
        (0..n.types.len() as u32)
            .map(Ty::Named)
            .chain((0..n.opts.len() as u32).map(Ty::Opt))
            .chain((0..n.ress.len() as u32).map(Ty::Res))
            .chain((0..n.lists.len() as u32).map(Ty::List))
            .chain((0..n.arrs.len() as u32).map(Ty::Arr))
            .chain((0..n.maps.len() as u32).map(Ty::Map))
            .collect()
    }

    fn type_def(&self, t: Ty, done: &mut HashSet<Ty>, c: &mut String) {
        if !matches!(t, Ty::Named(_) | Ty::Opt(_) | Ty::Res(_) | Ty::Arr(_)) || !done.insert(t) {
            return;
        }
        let members = |fs: &mut dyn Iterator<Item = (Ty, String)>| -> Vec<String> {
            fs.map(|(t, n)| format!("{} {n};", self.c_ty(t))).collect()
        };
        let (contained, body): (Vec<Ty>, String) = match t {
            Ty::Arr(i) => {
                let (v, n) = self.info.arrs[i as usize];
                (vec![v], format!("{} a[{n}];", self.c_ty(v)))
            }
            Ty::Opt(i) => {
                let v = self.info.opts[i as usize];
                (vec![v], format!("bool some; {} v;", self.c_ty(v)))
            }
            Ty::Res(i) => {
                let (v, e) = self.info.ress[i as usize];
                let value = if v == Ty::Void {
                    String::new()
                } else {
                    format!(" {} v;", self.c_ty(v))
                };
                (vec![v, e], format!("bool ok;{value} {} e;", self.c_ty(e)))
            }
            Ty::Named(i) => {
                let d = &self.info.types[i as usize];
                let contained: Vec<Ty> = match &d.body {
                    TypeBody::Struct(fs) => fs.iter().map(|f| f.1).collect(),
                    TypeBody::Enum(vs) => vs
                        .iter()
                        .flat_map(|v| v.1.iter().map(|f| f.0))
                        .filter(|&f| !self.boxed(i, f))
                        .collect(),
                };
                let body = match &d.body {
                    TypeBody::Struct(fs) => {
                        let mut m = members(&mut fs.iter().map(|f| (f.1, format!("f_{}", f.0))));
                        if m.is_empty() {
                            m.push("char unused;".into());
                        }
                        m.join(" ")
                    }
                    TypeBody::Enum(vs) => {
                        let arms: Vec<String> = vs
                            .iter()
                            .enumerate()
                            .filter(|(_, v)| !v.1.is_empty())
                            .map(|(k, v)| {
                                let m: Vec<String> = (v.1.iter().enumerate())
                                    .map(|(j, f)| {
                                        let star = if self.boxed(i, f.0) { " *" } else { " " };
                                        format!("{}{star}f{j};", self.c_ty(f.0))
                                    })
                                    .collect();
                                format!("struct {{ {} }} v{k};", m.join(" "))
                            })
                            .collect();
                        if arms.is_empty() {
                            "int64_t tag;".into()
                        } else {
                            format!("int64_t tag; union {{ {} }} u;", arms.join(" "))
                        }
                    }
                };
                (contained, body)
            }
            _ => return,
        };
        for u in contained {
            self.type_def(u, done, c);
        }
        let n = self.c_ty(t);
        let _ = writeln!(c, "struct {n} {{ {body} }};");
    }

    /// Every type that has helpers.
    fn composites(&self) -> Vec<Ty> {
        self.universe()
            .into_iter()
            .filter(|t| Self::composite(*t))
            .collect()
    }

    /// Prototypes of the helpers of every type.
    pub(super) fn helper_protos(&self) -> String {
        let mut c = String::new();
        for t in self.universe() {
            let _ = writeln!(c, "DIF_CONST dif_ty {};", self.dty(t));
        }
        for t in self.composites() {
            let (n, ct) = (self.sfx(t), self.c_ty(t));
            let _ = writeln!(c, "static inline bool dif_eq_{n}({ct} a, {ct} b);");
            let _ = writeln!(c, "static inline void dif_put_{n}(dif_buf *b, {ct} v);");
            let _ = writeln!(c, "static inline void dif_print_{n}({ct} v);");
            let _ = writeln!(
                c,
                "static inline void dg_inc_{n}(const dif_ty *t, const void *p);"
            );
            let _ = writeln!(
                c,
                "static inline void dg_dec_{n}(const dif_ty *t, void *p);"
            );
        }
        c
    }

    /// `a == b` for values of type `t` (borrowed operands, each used once).
    pub(super) fn eq(&self, t: Ty, a: &str, b: &str) -> String {
        match t {
            Ty::Str => format!("dif_eq({a}, {b})"),
            t if Self::composite(t) => format!("dif_eq_{}({a}, {b})", self.sfx(t)),
            Ty::List(i) => {
                format!(
                    "dif_list_eq(&{}, {a}, {b})",
                    self.dty(self.info.lists[i as usize])
                )
            }
            Ty::Map(i) => {
                let (k, v) = self.info.maps[i as usize];
                format!("dif_map_eq(&{}, &{}, {a}, {b})", self.dty(k), self.dty(v))
            }
            Ty::Arr(i) => {
                let (e, n) = self.info.arrs[i as usize];
                format!(
                    "({{ {ct} x_ = ({a}); {ct} y_ = ({b}); dif_arr_eq(&{}, x_.a, {n}, y_.a, {n}); }})",
                    self.dty(e),
                    ct = self.c_ty(t)
                )
            }
            _ => format!("({a} == {b})"),
        }
    }

    /// A statement appending the text of `v` to buffer `b`; a `str` or `rune` inside a
    /// collection, struct or enum is quoted (R28).
    pub(super) fn put(&self, t: Ty, b: &str, v: &str, quoted: bool) -> String {
        match t {
            Ty::Int | Ty::IntLit => format!("dif_put_i64({b}, {v});"),
            Ty::Num(Num::F64) | Ty::FloatLit => format!("dif_put_f64({b}, {v});"),
            Ty::Num(n) if n.signed() => format!("dif_put_i64({b}, (int64_t)({v}));"),
            Ty::Num(_) => format!("dif_put_u64({b}, (uint64_t)({v}));"),
            Ty::Bool => format!("dif_put_bool({b}, {v});"),
            Ty::Rune if quoted => format!("dif_put_rune_quoted({b}, {v});"),
            Ty::Rune => format!("dif_put_rune({b}, {v});"),
            Ty::Str if quoted => format!("dif_put_quoted({b}, {v});"),
            Ty::Str => format!("dif_put_str({b}, {v});"),
            t if Self::composite(t) => format!("dif_put_{}({b}, {v});", self.sfx(t)),
            Ty::List(i) => {
                format!(
                    "dif_list_put(&{}, {b}, {v});",
                    self.dty(self.info.lists[i as usize])
                )
            }
            Ty::Map(i) => {
                let (k, e) = self.info.maps[i as usize];
                format!("dif_map_put(&{}, &{}, {b}, {v});", self.dty(k), self.dty(e))
            }
            Ty::Arr(i) => {
                let (e, n) = self.info.arrs[i as usize];
                format!("dif_arr_put(&{}, {b}, ({v}).a, {n});", self.dty(e))
            }
            _ => format!("dif_put_c({b}, \"?\");"),
        }
    }

    pub(super) fn lit(b: &str, s: &str) -> String {
        format!(
            "dif_put({b}, {}, {});",
            super::c_string(s.as_bytes()),
            s.len()
        )
    }

    /// Statements that give the counted members of the value at `p` (a `const T *` or
    /// `T *`) one more owner, or release them.
    fn members(&self, t: Ty, p: &str, release: bool) -> String {
        let one = |t: Ty, place: &str| -> String {
            if !self.managed(t) {
                return String::new();
            }
            if release {
                format!("    {};\n", self.drop_e(t, place))
            } else {
                format!("    (void){};\n", self.copy_in_place(t, place))
            }
        };
        let mut c = String::new();
        match t {
            Ty::Opt(i) => {
                let inner = one(self.info.opts[i as usize], &format!("{p}->v"));
                if !inner.is_empty() {
                    let _ = write!(c, "    if ({p}->some) {{\n{inner}    }}\n");
                }
            }
            Ty::Res(i) => {
                let (v, e) = self.info.ress[i as usize];
                let vs = if v == Ty::Void {
                    String::new()
                } else {
                    one(v, &format!("{p}->v"))
                };
                let es = one(e, &format!("{p}->e"));
                if !vs.is_empty() || !es.is_empty() {
                    let _ = write!(c, "    if ({p}->ok) {{\n{vs}    }} else {{\n{es}    }}\n");
                }
            }
            Ty::Named(id) => match &self.info.types[id as usize].body {
                TypeBody::Struct(fs) => {
                    for (f, ft) in fs {
                        c.push_str(&one(*ft, &format!("{p}->f_{f}")));
                    }
                }
                TypeBody::Enum(vs) => {
                    for (k, v) in vs.iter().enumerate() {
                        let mut body = String::new();
                        for (j, f) in v.1.iter().enumerate() {
                            let place = format!("{p}->u.v{k}.f{j}");
                            if self.boxed(id, f.0) {
                                let _ = if release {
                                    writeln!(body, "    dif_box_dec(&{}, {place});", self.dty(f.0))
                                } else {
                                    writeln!(body, "    dif_box_inc({place});")
                                };
                            } else {
                                body.push_str(&one(f.0, &place));
                            }
                        }
                        if !body.is_empty() {
                            let _ = write!(c, "    if ({p}->tag == {k}) {{\n{body}    }}\n");
                        }
                    }
                }
            },
            _ => {}
        }
        c
    }

    /// `copy` of a value in a place: raises the count, discards the copy.
    fn copy_in_place(&self, t: Ty, place: &str) -> String {
        match t {
            Ty::Str | Ty::List(_) | Ty::Map(_) => self.copy(t, place),
            t => format!("dif_inc(&{}, &({place}))", self.dty(t)),
        }
    }

    /// The hash of a value of a composite type: mixes the members (R30).
    fn hash_body(&self, t: Ty) -> String {
        let h = |ty: Ty, place: &str| -> String {
            let d = self.dty(ty);
            format!("    h = dif_hash_mix(h, {d}.hash(&{d}, &({place})));\n")
        };
        let mut c = String::new();
        match t {
            Ty::Opt(i) => {
                c.push_str("    h = dif_hash_mix(h, v->some);\n");
                let _ = write!(
                    c,
                    "    if (v->some) {{\n{}    }}\n",
                    h(self.info.opts[i as usize], "v->v")
                );
            }
            Ty::Res(i) => {
                let (vt, e) = self.info.ress[i as usize];
                c.push_str("    h = dif_hash_mix(h, v->ok);\n");
                let vs = if vt == Ty::Void {
                    String::new()
                } else {
                    h(vt, "v->v")
                };
                let _ = write!(
                    c,
                    "    if (v->ok) {{\n{vs}    }} else {{\n{}    }}\n",
                    h(e, "v->e")
                );
            }
            Ty::Named(id) => match &self.info.types[id as usize].body {
                TypeBody::Struct(fs) => {
                    for (f, ft) in fs {
                        c.push_str(&h(*ft, &format!("v->f_{f}")));
                    }
                }
                TypeBody::Enum(vs) => {
                    c.push_str("    h = dif_hash_mix(h, (uint64_t)v->tag);\n");
                    for (k, v) in vs.iter().enumerate().filter(|(_, v)| !v.1.is_empty()) {
                        let mut body = String::new();
                        for (j, f) in v.1.iter().enumerate() {
                            body.push_str(&h(f.0, &self.fld(id, k, j, format!("v->u.v{k}.f{j}"))));
                        }
                        let _ = write!(c, "    if (v->tag == {k}) {{\n{body}    }}\n");
                    }
                }
            },
            _ => {}
        }
        c
    }

    /// The definitions of the helpers of every composite type.
    pub(super) fn helpers(&self) -> String {
        let mut c = String::new();
        for t in self.composites() {
            let (n, ct) = (self.sfx(t), self.c_ty(t));
            let _ = writeln!(c, "static inline bool dif_eq_{n}({ct} a, {ct} b) {{");
            let _ = writeln!(c, "    (void)a;\n    (void)b;");
            let mut text = String::new();
            match t {
                Ty::Opt(i) => {
                    let v = self.info.opts[i as usize];
                    let same = self.eq(v, "a.v", "b.v");
                    let _ = writeln!(c, "    return a.some == b.some && (!a.some || {same});\n}}");
                    let _ = writeln!(
                        text,
                        "    if (!v.some) {{\n        {}",
                        Self::lit("b", "none")
                    );
                    let _ = writeln!(text, "        return;\n    }}");
                    let _ = writeln!(text, "    {}", Self::lit("b", "some("));
                    let _ = writeln!(text, "    {}", self.put(v, "b", "v.v", true));
                    let _ = writeln!(text, "    {}", Self::lit("b", ")"));
                }
                Ty::Res(i) => {
                    let (v, e) = self.info.ress[i as usize];
                    let err = self.eq(e, "a.e", "b.e");
                    let same = if v == Ty::Void {
                        format!("(a.ok || {err})")
                    } else {
                        format!("(a.ok ? {} : {err})", self.eq(v, "a.v", "b.v"))
                    };
                    let _ = writeln!(c, "    return a.ok == b.ok && {same};\n}}");
                    let _ = writeln!(text, "    if (v.ok) {{");
                    let _ = writeln!(text, "        {}", Self::lit("b", "ok"));
                    if v != Ty::Void {
                        let _ = writeln!(text, "        {}", Self::lit("b", "("));
                        let _ = writeln!(text, "        {}", self.put(v, "b", "v.v", true));
                        let _ = writeln!(text, "        {}", Self::lit("b", ")"));
                    }
                    let _ = writeln!(text, "        return;\n    }}");
                    let _ = writeln!(text, "    {}", Self::lit("b", "err("));
                    let _ = writeln!(text, "    {}", self.put(e, "b", "v.e", true));
                    let _ = writeln!(text, "    {}", Self::lit("b", ")"));
                }
                Ty::Named(id) => {
                    let d = &self.info.types[id as usize];
                    match &d.body {
                        TypeBody::Struct(fs) => {
                            let all: Vec<String> = fs
                                .iter()
                                .map(|(f, t)| self.eq(*t, &format!("a.f_{f}"), &format!("b.f_{f}")))
                                .collect();
                            let all = if all.is_empty() {
                                "true".into()
                            } else {
                                all.join(" && ")
                            };
                            let _ = writeln!(c, "    return {all};\n}}");
                            let _ = writeln!(text, "    {}", Self::lit("b", &format!("{n}{{")));
                            for (k, (f, t)) in fs.iter().enumerate() {
                                let sep = if k == 0 { "" } else { ", " };
                                let _ = writeln!(
                                    text,
                                    "    {}",
                                    Self::lit("b", &format!("{sep}{f}: "))
                                );
                                let _ = writeln!(
                                    text,
                                    "    {}",
                                    self.put(*t, "b", &format!("v.f_{f}"), true)
                                );
                            }
                            let _ = writeln!(text, "    {}", Self::lit("b", "}"));
                        }
                        TypeBody::Enum(vs) => {
                            let _ = writeln!(c, "    if (a.tag != b.tag) return false;");
                            for (k, v) in vs.iter().enumerate().filter(|(_, v)| !v.1.is_empty()) {
                                let all: Vec<String> =
                                    v.1.iter()
                                        .enumerate()
                                        .map(|(j, f)| {
                                            let x = self.fld(id, k, j, format!("a.u.v{k}.f{j}"));
                                            let y = self.fld(id, k, j, format!("b.u.v{k}.f{j}"));
                                            self.eq(f.0, &x, &y)
                                        })
                                        .collect();
                                let _ = writeln!(
                                    c,
                                    "    if (a.tag == {k}) return {};",
                                    all.join(" && ")
                                );
                            }
                            let _ = writeln!(c, "    return true;\n}}");
                            // `io.IoErr` is written without its module in the text form (R28)
                            let shown = d.name.rsplit('.').next().unwrap_or(&d.name);
                            for (k, v) in vs.iter().enumerate() {
                                let _ = writeln!(text, "    if (v.tag == {k}) {{");
                                let _ = writeln!(
                                    text,
                                    "        {}",
                                    Self::lit("b", &format!("{shown}.{}", v.0))
                                );
                                if !v.1.is_empty() {
                                    let _ = writeln!(text, "        {}", Self::lit("b", "("));
                                }
                                for (j, f) in v.1.iter().enumerate() {
                                    let sep = if j == 0 { "" } else { ", " };
                                    let _ = writeln!(text, "        {}", Self::lit("b", sep));
                                    let x = self.fld(id, k, j, format!("v.u.v{k}.f{j}"));
                                    let _ =
                                        writeln!(text, "        {}", self.put(f.0, "b", &x, true));
                                }
                                if !v.1.is_empty() {
                                    let _ = writeln!(text, "        {}", Self::lit("b", ")"));
                                }
                                let _ = writeln!(text, "    }}");
                            }
                        }
                    }
                }
                _ => {}
            }
            let _ = writeln!(c, "static inline void dif_put_{n}(dif_buf *b, {ct} v) {{");
            let _ = writeln!(c, "    (void)v;");
            c.push_str(&text);
            let _ = writeln!(c, "}}");
            let _ = writeln!(c, "static inline void dif_print_{n}({ct} v) {{");
            let _ = writeln!(c, "    dif_buf b = dif_buf_new();");
            let _ = writeln!(c, "    dif_put_{n}(&b, v);");
            let _ = writeln!(c, "    dif_print_buf(&b);\n}}");
            // for the descriptor: counted members, equality, text form, hash
            let _ = writeln!(
                c,
                "static inline void dg_inc_{n}(const dif_ty *t, const void *p) {{"
            );
            let _ = writeln!(c, "    (void)t;\n    (void)p;");
            c.push_str(&self.members(t, &format!("((const {ct} *)p)"), false));
            let _ = writeln!(c, "}}");
            let _ = writeln!(
                c,
                "static inline void dg_dec_{n}(const dif_ty *t, void *p) {{"
            );
            let _ = writeln!(c, "    (void)t;\n    (void)p;");
            c.push_str(&self.members(t, &format!("(({ct} *)p)"), true));
            let _ = writeln!(c, "}}");
            let _ = writeln!(
                c,
                "static inline bool dg_eq_{n}(const dif_ty *t, const void *a, const void *b) {{\n    (void)t;\n    return dif_eq_{n}(*(const {ct} *)a, *(const {ct} *)b);\n}}"
            );
            let _ = writeln!(
                c,
                "static inline void dg_put_{n}(const dif_ty *t, dif_buf *b, const void *p) {{\n    (void)t;\n    dif_put_{n}(b, *(const {ct} *)p);\n}}"
            );
            let _ = writeln!(
                c,
                "static inline uint64_t dg_hash_{n}(const dif_ty *t, const void *p) {{\n    (void)t;\n    const {ct} *v = (const {ct} *)p;\n    (void)v;\n    uint64_t h = 7;"
            );
            c.push_str(&self.hash_body(t));
            let _ = writeln!(c, "    return h;\n}}");
        }
        c
    }

    /// The descriptors of all types (the helpers above and the runtime's generic ones).
    pub(super) fn descriptors(&self) -> String {
        let mut c = String::new();
        for t in self.universe() {
            let d = self.dty(t);
            match t {
                Ty::List(i) => {
                    let e = self.dty(self.info.lists[i as usize]);
                    let _ = writeln!(c, "DIF_CONST dif_ty {d} = DIF_LIST_TY(&{e});");
                }
                Ty::Arr(i) => {
                    let (e, n) = self.info.arrs[i as usize];
                    let _ = writeln!(
                        c,
                        "DIF_CONST dif_ty {d} = DIF_ARRAY_TY(&{}, {n}, sizeof({}));",
                        self.dty(e),
                        self.c_ty(t)
                    );
                }
                Ty::Map(i) => {
                    let (k, v) = self.info.maps[i as usize];
                    let _ = writeln!(
                        c,
                        "DIF_CONST dif_ty {d} = DIF_MAP_TY(&{}, &{});",
                        self.dty(k),
                        self.dty(v)
                    );
                }
                t => {
                    let n = self.sfx(t);
                    let (inc, dec) = if self.managed(t) {
                        (format!("dg_inc_{n}"), format!("dg_dec_{n}"))
                    } else {
                        ("NULL".into(), "NULL".into())
                    };
                    let _ = writeln!(
                        c,
                        "DIF_CONST dif_ty {d} = {{sizeof({}), NULL, NULL, 0, {inc}, {dec}, dg_eq_{n}, dg_put_{n}, NULL, dg_hash_{n}}};",
                        self.c_ty(t)
                    );
                }
            }
        }
        c
    }
}
