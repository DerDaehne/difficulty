//! Builtin functions and methods (§4), conversions (R31), literals and loops over
//! collections, `io` and `os`: the part of the backend that mostly calls the runtime.

use std::fmt::Write;

use super::{Arg, Emitter};
use crate::ast::*;
use crate::checker::{Num, Ty};
use crate::diag::Pos;

/// An integer type: `(bits, signed)`.
fn int_kind(t: Ty) -> Option<(u32, bool)> {
    match t {
        Ty::Int | Ty::IntLit => Some((64, true)),
        Ty::Num(n) if n != Num::F64 => Some((n.bits(), n.signed())),
        _ => None,
    }
}

impl<'a> Emitter<'a> {
    /// The C text of an optional value that is `some(v)` if `cond` holds.
    fn some_if(&self, opt: Ty, setup: &str, cond: &str, v: &str) -> String {
        format!(
            "({{ {setup} {} o_ = {{0}}; if ({cond}) {{ o_.some = true; o_.v = {v}; }} o_; }})",
            self.c_ty(opt)
        )
    }

    // ---- print, assert, repeat ----

    pub(super) fn print(&mut self, a: &'a Expr, err: bool) -> String {
        let t = self.ty(a);
        let v = self.bor(a);
        match t {
            Ty::Str if !err => format!("dif_print_str({v})"),
            Ty::Str => format!("dif_eprint_str({v})"),
            Ty::Int if !err => format!("dif_print_int({v})"),
            Ty::Bool if !err => format!("dif_print_bool({v})"),
            t if Self::composite(t) && !err => format!("dif_print_{}({v})", self.sfx(t)),
            t => {
                let put = self.put(t, "&b_", &v, false);
                let out = if err {
                    "dif_eprint_buf"
                } else {
                    "dif_print_buf"
                };
                format!("({{ dif_buf b_ = dif_buf_new(); {put} {out}(&b_); }})")
            }
        }
    }

    pub(super) fn assert(&mut self, e: &'a Expr, a: &'a Expr) -> String {
        let c = self.expr(a);
        let text = super::c_string(self.text_of(a).as_bytes());
        format!(
            "(({c}) ? (void)0 : dif_assert_fail({text}, {}, {}))",
            e.pos.line, e.pos.col
        )
    }

    pub(super) fn repeat(&mut self, e: &'a Expr, args: &'a [Expr]) -> String {
        let Ty::List(i) = self.ty(e) else {
            unreachable!("repeat without a list type")
        };
        let el = self.info.lists[i as usize];
        let items = [Arg::Val(&args[0], el), Arg::Val(&args[1], Ty::Int)];
        let (pre, v) = self.ordered(&items);
        let drop = self.drop_e(el, "x_");
        let body = format!(
            "({{ {} x_ = {}; dif_list *r_ = dif_list_repeat(&{}, &x_, {}, {}, {}); {drop}; r_; }})",
            self.c_ty(el),
            v[0],
            self.dty(el),
            v[1],
            e.pos.line,
            e.pos.col
        );
        Self::with_pre(pre, body)
    }

    // ---- literals and index ----

    pub(super) fn list_lit(&mut self, e: &'a Expr, items: &'a [Expr]) -> String {
        let t = self.ty(e);
        let el = self.elem_of(t);
        if items.is_empty() {
            return "NULL".into();
        }
        let ct = self.c_ty(el);
        let mut c = String::new();
        if let Ty::Arr(_) = t {
            let _ = write!(c, "({{ {} a_ = {{0}}; ", self.c_ty(t));
            for (k, x) in items.iter().enumerate() {
                let v = self.at(el, x);
                let _ = write!(c, "a_.a[{k}] = {v}; ");
            }
            c.push_str("a_; })");
            return c;
        }
        let _ = write!(c, "({{ {ct} e_[{}]; ", items.len());
        for (k, x) in items.iter().enumerate() {
            let v = self.at(el, x);
            let _ = write!(c, "e_[{k}] = {v}; ");
        }
        let _ = write!(
            c,
            "dif_list_from(&{}, e_, {}); }})",
            self.dty(el),
            items.len()
        );
        c
    }

    pub(super) fn map_lit(&mut self, e: &'a Expr, entries: &'a [(Expr, Expr)]) -> String {
        let Ty::Map(i) = self.ty(e) else {
            unreachable!("a map literal without a map type")
        };
        if entries.is_empty() {
            return "NULL".into();
        }
        let (kt, vt) = self.info.maps[i as usize];
        let t = self.ty(e);
        let m = self.mtemp(t);
        let mut c = String::from("({ ");
        for (k, v) in entries {
            let (ko, vo) = (self.at(kt, k), self.at(vt, v));
            let _ = write!(
                c,
                "{{ {} k_ = {ko}; {} v_ = {vo}; dif_map_set(&{}, &{}, &{m}, &k_, &v_, {}, {}); }} ",
                self.c_ty(kt),
                self.c_ty(vt),
                self.dty(kt),
                self.dty(vt),
                k.pos.line,
                k.pos.col
            );
        }
        let _ = write!(c, "{}; }})", Self::take_temp(&m));
        c
    }

    /// `m[k]`: `some(v)` or `none`.
    pub(super) fn map_get(
        &mut self,
        e: &'a Expr,
        base: &'a Expr,
        idx: &'a Expr,
        bp: crate::diag::Pos,
    ) -> String {
        let Ty::Map(i) = self.ty(base) else {
            unreachable!("an index of a map")
        };
        let (kt, vt) = self.info.maps[i as usize];
        let (pre, v) = self.ordered(&[Arg::Ref(base), Arg::Ref(idx)]);
        let copy = self.copy(vt, "*p_");
        let body = format!(
            "({{ {kc} k_ = {}; const {vc} *p_ = (const {vc} *)dif_map_get(&{}, &{}, {}, &k_, {}, {}); {} }})",
            v[1],
            self.dty(kt),
            self.dty(vt),
            v[0],
            bp.line,
            bp.col,
            {
                let opt = self.c_ty(self.ty(e));
                format!("{opt} o_ = {{0}}; if (p_) {{ o_.some = true; o_.v = {copy}; }} o_;")
            },
            kc = self.c_ty(kt),
            vc = self.c_ty(vt),
        );
        Self::with_pre(pre, body)
    }

    // ---- conversions T(x) (R31) ----

    pub(super) fn convert(&mut self, e: &'a Expr, a: &'a Expr) -> String {
        let (tt, st) = (self.ty(e), self.ty(a));
        let v = self.expr(a);
        if tt == st {
            return v;
        }
        let (l, c) = (e.pos.line, e.pos.col);
        let ct = self.c_ty(tt);
        match (tt, st) {
            (Ty::Str, Ty::Rune) => format!("dif_rune_str({v})"),
            (Ty::Rune, s) => match int_kind(s) {
                Some((_, true)) => format!("dif_rune_s((int64_t)({v}), {l}, {c})"),
                _ => format!("dif_rune_u((uint64_t)({v}), {l}, {c})"),
            },
            (Ty::Num(Num::F64), s) => match int_kind(s) {
                Some((_, true)) => format!("dif_s2f((int64_t)({v}), {l}, {c})"),
                _ => format!("dif_u2f((uint64_t)({v}), {l}, {c})"),
            },
            (t, Ty::Num(Num::F64)) => {
                let (bits, sgn) = int_kind(t).unwrap_or((64, true));
                let f = if sgn { "dif_f2s" } else { "dif_f2u" };
                format!("(({ct}){f}({v}, {bits}, {sgn}, {l}, {c}))")
            }
            (t, s) => {
                let (bits, sgn) = int_kind(t).unwrap_or((64, true));
                match int_kind(s) {
                    Some((_, true)) => {
                        format!("(({ct})dif_conv_s((int64_t)({v}), {bits}, {sgn}, {l}, {c}))")
                    }
                    _ => format!("(({ct})dif_conv_u((uint64_t)({v}), {bits}, {sgn}, {l}, {c}))"),
                }
            }
        }
    }

    // ---- methods of builtin types ----

    pub(super) fn builtin_method(
        &mut self,
        e: &'a Expr,
        base: &'a Expr,
        m: &str,
        args: &'a [Expr],
    ) -> String {
        let bt = self.ty(base);
        let (l, c) = self.name_pos(base);
        match bt {
            Ty::Str => self.str_method(e, base, m, args, (l, c)),
            Ty::Rune => {
                let v = self.expr(base);
                if m == "to_digit" {
                    let setup = format!("uint32_t r_ = {v};");
                    let cond = "r_ >= '0' && r_ <= '9'";
                    return self.some_if(self.ty(e), &setup, cond, "(int64_t)(r_ - '0')");
                }
                format!("dif_rune_{m}({v})")
            }
            Ty::Num(Num::F64) => {
                let mut items = vec![Arg::Val(base, bt)];
                items.extend(args.iter().map(|a| Arg::Val(a, bt)));
                let (pre, v) = self.ordered(&items);
                let f = match m {
                    "abs" => "fabs",
                    "min" => "fmin",
                    "max" => "fmax",
                    f => f,
                };
                Self::with_pre(pre, format!("{f}({})", v.join(", ")))
            }
            Ty::List(_) | Ty::Arr(_) => self.seq_method(e, base, m, args, (l, c)),
            Ty::Map(_) => self.map_method(base, m, args, (l, c)),
            t => self.int_method(e, base, t, m, args, (l, c)),
        }
    }

    fn int_method(
        &mut self,
        e: &'a Expr,
        base: &'a Expr,
        t: Ty,
        m: &str,
        args: &'a [Expr],
        at: (usize, usize),
    ) -> String {
        let mut items = vec![Arg::Val(base, t)];
        items.extend(args.iter().map(|a| Arg::Val(a, t)));
        let (pre, v) = self.ordered(&items);
        let ct = self.c_ty(t);
        let body = match m {
            "abs" => {
                let min = match int_kind(t) {
                    Some((64, _)) => "INT64_MIN".to_string(),
                    Some((b, _)) => format!("INT{b}_MIN"),
                    None => "0".into(),
                };
                format!(
                    "(({ct})dif_abs((int64_t)({}), {min}, {}, {}))",
                    v[0], at.0, at.1
                )
            }
            "min" | "max" => {
                let o = if m == "min" { "<" } else { ">" };
                format!(
                    "({{ {ct} a_ = {}; {ct} b_ = {}; a_ {o} b_ ? a_ : b_; }})",
                    v[0], v[1]
                )
            }
            _ => {
                // to_T(): some(T(self)) if the value fits
                let Ty::Opt(i) = self.ty(e) else {
                    unreachable!("to_T without an optional type")
                };
                let to = self.info.opts[i as usize];
                let (bits, sgn) = int_kind(to).unwrap_or((64, true));
                let fits = match int_kind(t) {
                    Some((_, true)) => format!("dif_fits_s((int64_t)v_, {bits}, {sgn})"),
                    _ => format!("dif_fits_u((uint64_t)v_, {bits}, {sgn})"),
                };
                let setup = format!("{ct} v_ = {};", v[0]);
                self.some_if(
                    Ty::Opt(i),
                    &setup,
                    &fits,
                    &format!("(({})v_)", self.c_ty(to)),
                )
            }
        };
        Self::with_pre(pre, body)
    }

    fn str_method(
        &mut self,
        e: &'a Expr,
        base: &'a Expr,
        m: &str,
        args: &'a [Expr],
        at: (usize, usize),
    ) -> String {
        let mut items = vec![Arg::Ref(base)];
        for a in args {
            items.push(match self.ty(a) {
                Ty::Str => Arg::Ref(a),
                t => Arg::Val(a, t),
            });
        }
        let (pre, v) = self.ordered(&items);
        let (l, c) = at;
        let body = match m {
            "len" => format!("({}.n)", v[0]),
            "is_empty" => format!("({}.n == 0)", v[0]),
            "runes" => format!("dif_str_runes({})", v[0]),
            "split" => format!("dif_str_split({}, {}, {l}, {c})", v[0], v[1]),
            "lines" => format!("dif_str_lines({})", v[0]),
            "trim" => format!("dif_str_trim({}, true, true)", v[0]),
            "trim_start" => format!("dif_str_trim({}, true, false)", v[0]),
            "trim_end" => format!("dif_str_trim({}, false, true)", v[0]),
            "lower" => format!("dif_str_lower({})", v[0]),
            "upper" => format!("dif_str_upper({})", v[0]),
            "contains" | "starts_with" | "ends_with" => format!("dif_{m}({}, {})", v[0], v[1]),
            "find" => {
                let setup = format!("int64_t r_ = dif_find({}, {});", v[0], v[1]);
                self.some_if(self.ty(e), &setup, "r_ >= 0", "r_")
            }
            "replace" => format!("dif_str_replace({}, {}, {}, {l}, {c})", v[0], v[1], v[2]),
            "repeat" => format!("dif_str_repeat({}, {}, {l}, {c})", v[0], v[1]),
            "slice" => format!("dif_str_slice({}, {}, {}, {l}, {c})", v[0], v[1], v[2]),
            "parse_int" => {
                let setup = "int64_t r_ = 0;";
                let cond = format!("dif_parse_int({}, &r_)", v[0]);
                self.some_if(self.ty(e), setup, &cond, "r_")
            }
            _ => {
                let setup = "double r_ = 0;";
                let cond = format!("dif_parse_f64({}, &r_)", v[0]);
                self.some_if(self.ty(e), setup, &cond, "r_")
            }
        };
        Self::with_pre(pre, body)
    }

    /// Methods of lists and arrays.
    fn seq_method(
        &mut self,
        e: &'a Expr,
        base: &'a Expr,
        m: &str,
        args: &'a [Expr],
        at: (usize, usize),
    ) -> String {
        let bt = self.ty(base);
        let el = self.elem_of(bt);
        let (ect, d) = (self.c_ty(el), self.dty(el));
        let arr_n = match bt {
            Ty::Arr(i) => Some(self.info.arrs[i as usize].1),
            _ => None,
        };
        let (l, c) = at;
        let changes = matches!(
            m,
            "push" | "pop" | "insert" | "remove" | "sort" | "reverse" | "swap" | "sort_by"
        );
        let mut items = vec![if changes {
            Arg::Addr(base)
        } else {
            Arg::Ref(base)
        }];
        match m {
            "push" => items.push(Arg::Val(&args[0], el)),
            "insert" => {
                items.push(Arg::Val(&args[0], Ty::Int));
                items.push(Arg::Val(&args[1], el));
            }
            "contains" | "index_of" | "count" => items.push(Arg::Ref(&args[0])),
            "filter" | "map" | "sort_by" | "any" | "all" => {
                items.push(Arg::Val(&args[0], self.ty(&args[0])));
            }
            _ => items.extend(args.iter().map(|a| Arg::Val(a, Ty::Int))),
        }
        let (pre, v) = self.ordered(&items);
        // the list or array as the runtime takes it: (data, length) for an array
        let data = |s: &str| match arr_n {
            Some(n) => format!("({s}).a, {n}"),
            None => s.to_string(),
        };
        let kind = if arr_n.is_some() { "arr" } else { "list" };
        let body = match m {
            "len" | "is_empty" => {
                let n = match arr_n {
                    Some(n) => format!("(({}, INT64_C({n})))", format_args!("(void)({})", v[0])),
                    None => format!("dif_list_len({})", v[0]),
                };
                if m == "len" {
                    n
                } else {
                    format!("({n} == 0)")
                }
            }
            "push" => format!("({{ {ect} x_ = {}; dif_list_push(&{d}, {}, &x_); }})", v[1], v[0]),
            "insert" => format!(
                "({{ int64_t i_ = {}; {ect} x_ = {}; dif_list_insert(&{d}, {}, i_, &x_, {l}, {c}); }})",
                v[1], v[2], v[0]
            ),
            "pop" => {
                let opt = self.c_ty(self.ty(e));
                format!(
                    "({{ {ect} x_ = {{0}}; {opt} o_ = {{0}}; if (dif_list_pop(&{d}, {}, &x_)) {{ o_.some = true; o_.v = x_; }} o_; }})",
                    v[0]
                )
            }
            "remove" => format!(
                "({{ {ect} x_ = {{0}}; dif_list_remove(&{d}, {}, {}, &x_, {l}, {c}); x_; }})",
                v[0], v[1]
            ),
            "first" | "last" => {
                let opt = self.c_ty(self.ty(e));
                let last = m == "last";
                match arr_n {
                    Some(n) => {
                        let k = if last { n - 1 } else { 0 };
                        let copy = self.copy(el, &format!("s_.a[{k}]"));
                        format!(
                            "({{ {} s_ = {}; {opt} o_ = {{0}}; o_.some = true; o_.v = {copy}; o_; }})",
                            self.c_ty(bt),
                            v[0]
                        )
                    }
                    None => {
                        let copy = self.copy(el, "*p_");
                        format!(
                            "({{ const {ect} *p_ = (const {ect} *)dif_list_{m}(&{d}, {}); {opt} o_ = {{0}}; if (p_) {{ o_.some = true; o_.v = {copy}; }} o_; }})",
                            v[0]
                        )
                    }
                }
            }
            "contains" => format!(
                "({{ {ect} x_ = {}; dif_{kind}_contains(&{d}, {}, &x_); }})",
                v[1],
                data(&v[0])
            ),
            "index_of" => {
                let setup = format!(
                    "{ect} x_ = {}; int64_t r_ = dif_{kind}_index_of(&{d}, {}, &x_);",
                    v[1],
                    data(&v[0])
                );
                self.some_if(self.ty(e), &setup, "r_ >= 0", "r_")
            }
            "count" => format!(
                "({{ {ect} x_ = {}; dif_{kind}_count(&{d}, {}, &x_); }})",
                v[1],
                data(&v[0])
            ),
            "swap" => match arr_n {
                Some(n) => format!("dif_arr_swap(&{d}, ({})->a, {n}, {}, {}, {l}, {c})", v[0], v[1], v[2]),
                None => format!("dif_list_swap(&{d}, {}, {}, {}, {l}, {c})", v[0], v[1], v[2]),
            },
            "sort" | "reverse" => match arr_n {
                Some(n) => format!("dif_arr_{m}(&{d}, ({})->a, {n})", v[0]),
                None => format!("dif_list_{m}(&{d}, {})", v[0]),
            },
            "slice" => match arr_n {
                Some(n) => format!(
                    "dif_arr_slice(&{d}, ({}).a, {n}, {}, {}, {l}, {c})",
                    v[0], v[1], v[2]
                ),
                None => format!("dif_list_slice(&{d}, {}, {}, {}, {l}, {c})", v[0], v[1], v[2]),
            },
            "filter" | "map" | "any" | "all" => {
                // one loop over the elements; the function gets a copy of each (spec 4.7)
                let fty = self.c_ty(self.ty(&args[0]));
                let elem = match arr_n {
                    Some(_) => format!("((const {ect} *)(s_.a))[i_]"),
                    None => format!("((const {ect} *)dif_list_data(s_))[i_]"),
                };
                let n = match arr_n {
                    Some(n) => format!("INT64_C({n})"),
                    None => "dif_list_len(s_)".to_string(),
                };
                let call = format!("f_.fn(f_.env, {})", self.copy(el, "p_[0]"));
                let head = format!(
                    "{fty} f_ = {}; {} s_ = {}; (void)s_;",
                    v[1],
                    self.c_ty(bt),
                    v[0]
                );
                let stop = match m {
                    "any" => " && !r_",
                    "all" => " && r_",
                    _ => "",
                };
                let lp = format!(
                    "for (int64_t i_ = 0, n_ = {n}; i_ < n_{stop}; i_++) {{ const {ect} *p_ = &{elem};"
                );
                match m {
                    "filter" => format!(
                        "({{ {head} dif_list *r_ = NULL; {lp} if ({call}) {{ {ect} x_ = {}; dif_list_push(&{d}, &r_, &x_); }} }} r_; }})",
                        self.copy(el, "p_[0]")
                    ),
                    "map" => {
                        let rt = self.elem_of(self.ty(e));
                        format!(
                            "({{ {head} dif_list *r_ = NULL; {lp} {} y_ = {call}; dif_list_push(&{}, &r_, &y_); }} r_; }})",
                            self.c_ty(rt),
                            self.dty(rt)
                        )
                    }
                    "any" => format!("({{ {head} bool r_ = false; {lp} r_ = {call}; }} r_; }})"),
                    _ => format!("({{ {head} bool r_ = true; {lp} r_ = {call}; }} r_; }})"),
                }
            }
            "sort_by" => {
                let Ty::Func(fi) = self.ty(&args[0]) else {
                    return String::new();
                };
                let ad = self.less_adapter(fi, el);
                let fty = self.c_ty(self.ty(&args[0]));
                match arr_n {
                    Some(n) => format!(
                        "({{ {fty} f_ = {}; dif_arr_sort_by(&{d}, ({})->a, {n}, {ad}, &f_); }})",
                        v[1], v[0]
                    ),
                    None => format!(
                        "({{ {fty} f_ = {}; dif_list_sort_by(&{d}, {}, {ad}, &f_); }})",
                        v[1], v[0]
                    ),
                }
            }
            _ => match arr_n {
                Some(n) => format!("dif_arr_join((const dif_str *)({}).a, {n}, {})", v[0], v[1]),
                None => format!("dif_list_join({}, {})", v[0], v[1]),
            },
        };
        Self::with_pre(pre, body)
    }

    /// The C function that lets the runtime call `less` of function type `fi` on two elements.
    fn less_adapter(&mut self, fi: u32, el: Ty) -> String {
        let name = format!("dsb_{fi}");
        if self.wrappers.insert(name.clone()) {
            let (ect, fty) = (self.c_ty(el), self.c_ty(Ty::Func(fi)));
            let (a, b) = (
                self.copy(el, &format!("*(const {ect} *)a")),
                self.copy(el, &format!("*(const {ect} *)b")),
            );
            let call = format!("f->fn(f->env, {a}, {b})");
            let header = format!("static bool {name}(void *ctx, const void *a, const void *b)");
            let _ = writeln!(self.lit_protos, "{header};");
            let _ = writeln!(
                self.lit_defs,
                "{header} {{ {fty} *f = ctx; return {call}; }}"
            );
        }
        name
    }

    fn map_method(
        &mut self,
        base: &'a Expr,
        m: &str,
        args: &'a [Expr],
        at: (usize, usize),
    ) -> String {
        let Ty::Map(i) = self.ty(base) else {
            unreachable!("map method on a non-map")
        };
        let (kt, vt) = self.info.maps[i as usize];
        let (kd, vd) = (self.dty(kt), self.dty(vt));
        let (l, c) = at;
        let mut items = vec![if matches!(m, "remove" | "add") {
            Arg::Addr(base)
        } else {
            Arg::Ref(base)
        }];
        if m == "add" {
            items.push(Arg::Val(&args[0], kt));
            items.push(Arg::Val(&args[1], vt));
        } else {
            items.extend(args.iter().map(Arg::Ref));
        }
        let (pre, v) = self.ordered(&items);
        let kc = self.c_ty(kt);
        let body = match m {
            "len" => format!("dif_map_len({})", v[0]),
            "has" => format!(
                "({{ {kc} k_ = {}; dif_map_has(&{kd}, &{vd}, {}, &k_, {l}, {c}); }})",
                v[1], v[0]
            ),
            "remove" => format!(
                "({{ {kc} k_ = {}; dif_map_remove(&{kd}, &{vd}, {}, &k_, {l}, {c}); }})",
                v[1], v[0]
            ),
            "add" => {
                let vc = self.c_ty(vt);
                let sum = self.op(
                    "+",
                    vt,
                    "*p_".into(),
                    "x_".into(),
                    Pos {
                        line: l as u32,
                        col: c as u32,
                        offset: 0,
                    },
                );
                format!(
                    "({{ {kc} k_ = {}; {vc} x_ = {}; {vc} *p_ = ({vc} *)dif_map_slot(&{kd}, &{vd}, {}, &k_, {l}, {c}); *p_ = {sum}; }})",
                    v[1], v[2], v[0]
                )
            }
            "keys" => format!("dif_map_keys(&{kd}, &{vd}, {})", v[0]),
            _ => format!("dif_map_values(&{kd}, &{vd}, {})", v[0]),
        };
        Self::with_pre(pre, body)
    }

    // ---- io and os ----

    pub(super) fn module_call(
        &mut self,
        e: &'a Expr,
        base: &'a Expr,
        module: &str,
        name: &str,
        args: &'a [Expr],
    ) -> String {
        let (l, c) = self.name_pos(base);
        let r = self.c_ty(self.ty(e));
        let items: Vec<Arg<'a>> = args
            .iter()
            .map(|a| match self.ty(a) {
                Ty::Int => Arg::Val(a, Ty::Int),
                _ => Arg::Ref(a),
            })
            .collect();
        if name == "eprint" {
            return self.print(&args[0], true);
        }
        let (pre, v) = self.ordered(&items);
        let body = match (module, name) {
            ("io", "read_file") => format!(
                "({{ dif_str o_; dif_ioerr x_; {r} r_ = {{0}}; if (dif_io_read_file({}, &o_, &x_)) {{ r_.ok = true; r_.v = o_; }} else {{ r_.e = dif_ioerr_val(x_); }} r_; }})",
                v[0]
            ),
            ("io", "write_file") => format!(
                "({{ dif_ioerr x_; {r} r_ = {{0}}; if (dif_io_write_file({}, {}, &x_)) {{ r_.ok = true; }} else {{ r_.e = dif_ioerr_val(x_); }} r_; }})",
                v[0], v[1]
            ),
            ("io", "read_lines") => format!(
                "({{ dif_list *o_; dif_ioerr x_; {r} r_ = {{0}}; if (dif_io_read_lines({}, &o_, &x_)) {{ r_.ok = true; r_.v = o_; }} else {{ r_.e = dif_ioerr_val(x_); }} r_; }})",
                v[0]
            ),
            ("io", "write_lines") => format!(
                "({{ dif_ioerr x_; {r} r_ = {{0}}; if (dif_io_write_lines({}, {}, &x_)) {{ r_.ok = true; }} else {{ r_.e = dif_ioerr_val(x_); }} r_; }})",
                v[0], v[1]
            ),
            ("io", "read_line") => format!(
                "({{ dif_str o_; dif_ioerr x_; {r} r_ = {{0}}; int k_ = dif_io_read_line(&o_, &x_); if (k_ > 0) {{ r_.ok = true; r_.v.some = true; r_.v.v = o_; }} else if (k_ == 0) {{ r_.ok = true; }} else {{ r_.e = dif_ioerr_val(x_); }} r_; }})"
            ),
            ("os", "args") => format!("dif_os_args({l}, {c})"),
            ("os", "exit") => format!("dif_os_exit({}, {l}, {c})", v[0]),
            _ => {
                let setup = "dif_str s_ = DIF_EMPTY;";
                let cond = format!("dif_os_env({}, &s_, {l}, {c})", v[0]);
                self.some_if(self.ty(e), setup, &cond, "s_")
            }
        };
        Self::with_pre(pre, body)
    }

    // ---- loops over collections, results and stdin ----

    pub(super) fn for_stmt(
        &mut self,
        vars: &'a [Param],
        iter: &'a Expr,
        body: &'a Block,
        d: usize,
    ) {
        let (src, tried) = match &iter.kind {
            ExprKind::Try(x, or) => (&**x, Some(or.as_ref())),
            _ => (iter, None),
        };
        if let ExprKind::Call(f, args) = &src.kind {
            match &f.kind {
                ExprKind::Ident(n) if n == "range" => return self.for_range(vars, args, body, d),
                ExprKind::Field(b, n) if n == "lines" && self.module_of(b) == Some("io") => {
                    return self.for_lines(vars, tried, iter, body, d);
                }
                _ => {}
            }
        }
        // `xs.slice(a, b)`: an index loop over `xs` itself, no copy of the part
        let part = match &src.kind {
            ExprKind::Call(f, args) if tried.is_none() => match (&f.kind, args.as_slice()) {
                (ExprKind::Field(b, m), [lo, hi])
                    if m == "slice" && matches!(self.ty(b), Ty::List(_)) =>
                {
                    Some((&**b, lo, hi))
                }
                _ => None,
            },
            _ => None,
        };
        // `s.runes()`: decode in place, no list of runes
        let runes = match &src.kind {
            ExprKind::Call(f, args) if tried.is_none() && args.is_empty() => match &f.kind {
                ExprKind::Field(b, m) if m == "runes" && self.ty(b) == Ty::Str => Some(&**b),
                _ => None,
            },
            _ => None,
        };
        let st = runes.map_or(self.ty(src), |_| Ty::Str);
        let s = format!("s{}", self.uid);
        let i = format!("i{}", self.uid);
        self.uid += 1;
        self.line(d, "{");
        self.push_scope();
        let v = self.bor(runes.unwrap_or_else(|| part.map_or(src, |p| p.0)));
        let (lo, hi) = (format!("lo{}", &s[1..]), format!("hi{}", &s[1..]));
        match st {
            Ty::List(_) => self.line(d + 1, &format!("dif_list *{s} = dif_list_inc({v});")),
            Ty::Map(_) => self.line(d + 1, &format!("dif_map *{s} = dif_map_inc({v});")),
            t => self.line(
                d + 1,
                &format!("{} {s} = {};", self.c_ty(t), self.copy(t, &v)),
            ),
        }
        self.own_local(st, s.clone());
        if let Some((base, a, b)) = part {
            let (ca, cb) = (self.at(Ty::Int, a), self.at(Ty::Int, b));
            let (l, c) = self.name_pos(base);
            self.line(d + 1, &format!("int64_t {lo} = {ca}, {hi} = {cb};"));
            self.line(
                d + 1,
                &format!(
                    "if ({lo} < 0 || {lo} > {hi} || {hi} > dif_list_len({s})) dif_panicf({l}, {c}, \"slice %\" PRId64 \"..%\" PRId64 \" out of bounds for length %\" PRId64, {lo}, {hi}, dif_list_len({s}));"
                ),
            );
        }
        let (n, kd, vd);
        match st {
            Ty::List(_) => {
                n = if part.is_some() {
                    hi.clone()
                } else {
                    format!("dif_list_len({s})")
                };
                (kd, vd) = (String::new(), String::new());
            }
            Ty::Arr(a) => {
                n = format!("INT64_C({})", self.info.arrs[a as usize].1);
                (kd, vd) = (String::new(), String::new());
            }
            Ty::Map(m) => {
                let (k, v) = self.info.maps[m as usize];
                n = format!("dif_map_cnt({s})");
                (kd, vd) = (self.dty(k), self.dty(v));
            }
            Ty::Str if runes.is_some() => {
                n = format!("{s}.n");
                (kd, vd) = (String::new(), String::new());
            }
            _ => unreachable!("a loop over something that is no collection"),
        }
        let start = if part.is_some() { lo.as_str() } else { "0" };
        if runes.is_some() {
            let k = format!("k{}", &s[1..]);
            self.line(
                d + 1,
                &format!("for (int64_t {i} = 0, n_ = {n}, {k} = 0; {i} < n_; {k}++) {{"),
            );
            self.line(d + 2, &format!("uint32_t c{k}; if ((unsigned char){s}.p[{i}] < 0x80) c{k} = (unsigned char){s}.p[{i}++]; else {i} += dif_utf8_step({s}.p + {i}, n_ - {i}, &c{k});"));
            self.line(d + 2, &format!("(void){k};"));
        } else {
            self.line(
                d + 1,
                &format!("for (int64_t {i} = {start}, n_ = {n}; {i} < n_; {i}++) {{"),
            );
        }
        // the elements of the iteration: (variable, type, expression of the element)
        let mut vals: Vec<(&Param, Ty, String)> = Vec::new();
        let mut tried_elem: Option<(Ty, String)> = None;
        match st {
            Ty::Map(m) => {
                let (kt, vt) = self.info.maps[m as usize];
                self.line(
                    d + 2,
                    &format!("if (!dif_map_key_at(&{kd}, &{vd}, {s}, {i})) continue;"),
                );
                let k = format!(
                    "(*(const {} *)dif_map_key_at(&{kd}, &{vd}, {s}, {i}))",
                    self.c_ty(kt)
                );
                let v = format!(
                    "(*(const {} *)dif_map_val_at(&{kd}, &{vd}, {s}, {i}))",
                    self.c_ty(vt)
                );
                vals.push((&vars[0], kt, k));
                vals.push((&vars[1], vt, v));
            }
            Ty::Str => {
                let k = format!("k{}", &s[1..]);
                if vars.len() == 2 {
                    vals.push((&vars[0], Ty::Int, k.clone()));
                }
                vals.push((&vars[vars.len() - 1], Ty::Rune, format!("c{k}")));
            }
            _ => {
                let el = self.elem_of(st);
                let x = match st {
                    Ty::List(_) => format!("((const {} *)dif_list_data({s}))[{i}]", self.c_ty(el)),
                    _ => format!("{s}.a[{i}]"),
                };
                if tried.is_some() {
                    tried_elem = Some((el, x));
                } else {
                    if vars.len() == 2 {
                        vals.push((&vars[0], Ty::Int, format!("({i} - {start})")));
                    }
                    let last = vars.len() - 1;
                    vals.push((&vars[last], el, x));
                }
            }
        }
        self.push_scope();
        self.loops.push(self.scopes.len() - 1);
        if let Some((el, x)) = tried_elem {
            // `for T x in try xs`: every element unpacked like `try` (R12)
            let y = format!("y{}", self.uid);
            self.uid += 1;
            let copy = self.copy(el, &x);
            self.line(d + 2, &format!("{} {y} = {copy};", self.c_ty(el)));
            let or = tried.flatten();
            let leave = self.try_leave(el, &y, or, iter.pos);
            let ok = if matches!(el, Ty::Res(_)) {
                "ok"
            } else {
                "some"
            };
            self.line(d + 2, &format!("if (!{y}.{ok}) {leave}"));
            if vars.len() == 2 {
                self.line(
                    d + 2,
                    &format!(
                        "int64_t d_{} = {i}; (void)d_{};",
                        vars[0].name, vars[0].name
                    ),
                );
            }
            let last = vars.len() - 1;
            let vt = self.ty_of(&vars[last].ty);
            self.line(
                d + 2,
                &format!(
                    "{} d_{} = {y}.v; (void)d_{};",
                    self.c_ty(vt),
                    vars[last].name,
                    vars[last].name
                ),
            );
            self.own_local(vt, format!("d_{}", vars[last].name));
        }
        for (p, t, x) in vals {
            let vt = self.ty_of(&p.ty);
            let copy = self.copy(t, &x);
            self.line(d + 2, &format!("{} d_{} = {copy};", self.c_ty(vt), p.name));
            self.line(d + 2, &format!("(void)d_{};", p.name));
            self.own_local(vt, format!("d_{}", p.name));
        }
        self.block(body, d + 3);
        let ends = matches!(
            body.stmts.last(),
            Some(Stmt::Return(..) | Stmt::Break(_) | Stmt::Continue(_))
        );
        self.pop_scope(d + 2, !ends);
        self.loops.pop();
        self.line(d + 1, "}");
        self.pop_scope(d + 1, true);
        self.line(d, "}");
    }

    /// `for T x in range(a, b)`: bounds once before the loop, the variable a fresh
    /// copy in every step (R11).
    fn for_range(&mut self, vars: &'a [Param], args: &'a [Expr], body: &'a Block, d: usize) {
        let (lo, hi) = match args {
            [n] => ("INT64_C(0)".to_string(), self.expr(n)),
            [a, b] => (self.expr(a), self.expr(b)),
            _ => return,
        };
        let (l, h, i) = (
            self.temp("int64_t".into(), "t"),
            self.temp("int64_t".into(), "t"),
            self.temp("int64_t".into(), "t"),
        );
        self.line(d, &format!("{l} = {lo};"));
        self.line(d, &format!("{h} = {hi};"));
        self.line(d, &format!("for ({i} = {l}; {i} < {h}; {i}++) {{"));
        let v = &vars[0];
        self.line(d + 1, &format!("int64_t d_{} = {i};", v.name));
        self.line(d + 1, &format!("(void)d_{};", v.name));
        self.loops.push(self.scopes.len());
        self.block(body, d + 2);
        self.loops.pop();
        self.line(d, "}");
    }

    /// `for … in [try] io.lines()`: one line per step, read when the step starts (R11).
    fn for_lines(
        &mut self,
        vars: &'a [Param],
        tried: Option<Option<&'a TryOr>>,
        iter: &'a Expr,
        body: &'a Block,
        d: usize,
    ) {
        let io_err = self
            .info
            .types
            .iter()
            .position(|t| t.name == "io.IoErr")
            .map_or(Ty::Err, |i| Ty::Named(i as u32));
        let res = self
            .info
            .ress
            .iter()
            .position(|r| *r == (Ty::Str, io_err))
            .map_or(Ty::Err, |i| Ty::Res(i as u32));
        let n = self.uid;
        self.uid += 1;
        let (y, i, k) = (format!("y{n}"), format!("i{n}"), format!("k{n}"));
        let rt = self.c_ty(res);
        self.line(d, "{");
        self.line(d + 1, &format!("int {k} = 1;"));
        self.line(d + 1, &format!("for (int64_t {i} = 0; {k} >= 0; {i}++) {{"));
        self.line(d + 2, "dif_str l_; dif_ioerr e_;");
        self.line(d + 2, &format!("(void){i};"));
        self.line(d + 2, &format!("{k} = dif_io_read_line(&l_, &e_);"));
        self.line(d + 2, &format!("if ({k} == 0) break;"));
        self.line(d + 2, &format!("{rt} {y} = {{0}};"));
        self.line(d + 2, &format!("if ({k} > 0) {{ {y}.ok = true; {y}.v = l_; }} else {{ {y}.e = dif_ioerr_val(e_); }}"));
        self.push_scope();
        self.loops.push(self.scopes.len() - 1);
        // the element is owned by the variable (or the failure leaves the function)
        let last = vars.len() - 1;
        if let Some(or) = tried {
            let leave = self.try_leave(res, &y, or, iter.pos);
            self.line(d + 2, &format!("if (!{y}.ok) {leave}"));
        }
        if vars.len() == 2 {
            self.line(
                d + 2,
                &format!(
                    "int64_t d_{} = {i}; (void)d_{};",
                    vars[0].name, vars[0].name
                ),
            );
        }
        let vt = self.ty_of(&vars[last].ty);
        let from = if tried.is_some() {
            format!("{y}.v")
        } else {
            y.clone()
        };
        self.line(
            d + 2,
            &format!(
                "{} d_{} = {from}; (void)d_{};",
                self.c_ty(vt),
                vars[last].name,
                vars[last].name
            ),
        );
        self.own_local(vt, format!("d_{}", vars[last].name));
        self.block(body, d + 3);
        let ends = matches!(
            body.stmts.last(),
            Some(Stmt::Return(..) | Stmt::Break(_) | Stmt::Continue(_))
        );
        self.pop_scope(d + 2, !ends);
        self.loops.pop();
        self.line(d + 1, "}");
        self.line(d, "}");
    }
}
