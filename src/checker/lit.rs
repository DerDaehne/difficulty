//! Function literals, blocks and function-typed parameters (R26, with R6 and R13 of part 2d).
//! A literal is checked like a function body of its own; the names it uses from outside are
//! captured by value, so it never changes anything outside.

use super::*;

impl<'a> Checker<'a> {
    fn local_idx(&self, name: &str) -> Option<usize> {
        self.scopes
            .iter()
            .rposition(|s| s.iter().any(|l| l.name == name))
    }

    /// `name` (a local) is used: every open literal that lies inside its scope captures it.
    pub(super) fn capture(&mut self, name: &str, ty: Ty) {
        let Some(idx) = self.local_idx(name) else {
            return;
        };
        for f in &mut self.frames {
            if f.base > idx && !f.captured.iter().any(|c| c.0 == name) {
                f.captured.push((name.to_string(), ty));
            }
        }
    }

    pub(super) fn capture_self(&mut self, ty: Ty) {
        for f in &mut self.frames {
            if !f.captured.iter().any(|c| c.0 == "self") {
                f.captured.push(("self".to_string(), ty));
            }
        }
    }

    /// A function name used in another way than a call or an argument (R26).
    pub(super) fn only_called(&mut self, name: &str, pos: Pos) {
        let msg = format!("`{name}` is a function and can only be called or passed as an argument");
        self.err("R26", pos, msg, None);
    }

    /// A change to `root` inside a literal: lost if `root` comes from outside it (R6).
    pub(super) fn lit_changed(&mut self, root: &str, pos: Pos) {
        let Some(fr) = self.frames.last() else {
            return;
        };
        let outside = root == "self" || self.local_idx(root).is_some_and(|i| i < fr.base);
        if !outside || fr.changed.iter().any(|r| r == root) {
            return;
        }
        if let Some(fr) = self.frames.last_mut() {
            fr.changed.push(root.to_string());
        }
        let msg = format!("the change to `{root}` is lost: `{root}` is a copy");
        let hint = "a function literal sees a copy; return the value or use a for loop";
        self.err("R6", pos, msg, Some(hint.to_string()));
    }

    /// `return` inside a literal: the value is the literal's, not the function's.
    pub(super) fn lit_ret(&mut self, want: Ty, value: Option<&'a Expr>, p: Pos) {
        match value {
            None if !self.no_value(want) && want != Ty::Err => {
                let msg = format!(
                    "the literal returns {}; give `return` a value",
                    self.tn(want)
                );
                let hint = format!("return {}", self.zero(want));
                self.err("R8", p, msg, Some(hint));
            }
            Some(e) if want == Ty::Void => {
                self.rvalue(e);
                self.err(
                    "R8",
                    e.pos,
                    "the literal returns nothing; remove the value",
                    None,
                );
            }
            Some(e) => {
                self.at_place(want, e, None);
            }
            None => {}
        }
    }

    /// Does the call just checked have an effect (R13)?
    pub(super) fn call_effect(&self, e: &Expr) -> bool {
        if self.call_lit.contains(&(e.pos.offset, e.end)) {
            return true;
        }
        match (&self.derived, self.last_callee) {
            (Some(d), Some(id)) => d[id].effect,
            (Some(_), None) => self.last_effect.unwrap_or(true),
            _ => false,
        }
    }

    /// Does the statement `e` pass a literal or a function with an effect?
    pub(super) fn lit_effect_in(&self, e: &Expr) -> bool {
        let inner = match &e.kind {
            ExprKind::Try(x, _) => x,
            _ => e,
        };
        self.call_lit.contains(&(inner.pos.offset, inner.end))
    }

    pub(super) fn convert_pure(&mut self, e: &'a Expr, name: &str, args: &'a [Expr]) -> (Ty, Val) {
        let r = self.convert(e, name, args);
        self.last_callee = None;
        self.last_effect = Some(false);
        r
    }

    /// `f(x)` for a function-typed parameter `f`.
    pub(super) fn call_param(
        &mut self,
        e: &'a Expr,
        f: &'a Expr,
        name: &str,
        fi: u32,
        args: &'a [Expr],
    ) -> (Ty, Val) {
        self.capture(name, Ty::Func(fi));
        self.exprs.insert((f.pos.offset, f.end), Ty::Func(fi));
        let (ret, ps) = self.ftys[fi as usize].clone();
        let params = (ps.iter().enumerate())
            .map(|(j, &(t, m))| (t, m, format!("argument {}", j + 1)))
            .collect();
        let ok = self.args_to(e, name, params, None, args, Vec::new());
        self.last_callee = None;
        self.last_effect = Some(false);
        if ok {
            (ret, Val::Runtime)
        } else {
            (Ty::Err, Val::Runtime)
        }
    }

    fn is_type_name(&self, n: &str) -> bool {
        self.type_ids.contains_key(n)
            || matches!(n, "int" | "bool" | "str" | "rune")
            || Num::from_name(n).is_some()
    }

    /// A literal where no argument is: an error, except `Name() { … }` of a function (R26).
    pub(super) fn func_lit_value(&mut self, e: &'a Expr, l: &'a FuncLit) -> (Ty, Val) {
        if let Some(alt) = &l.alt {
            if !self.is_type_name(&l.ret.name) {
                self.lit_calls.insert((e.pos.offset, e.end));
                return self.expr(alt);
            }
        }
        let msg = "a function literal can only be an argument of a call";
        let hint = "pass it directly as an argument of a call".to_string();
        self.err("R26", e.pos, msg, Some(hint));
        (Ty::Err, Val::Runtime)
    }

    /// An argument for a parameter of type `pt`: a literal, a block, the name of a function,
    /// a variant constructor or a function parameter when `pt` is a function type; anything
    /// else is an ordinary argument.
    pub(super) fn arg_value(&mut self, call: &'a Expr, callee: &str, pt: Ty, a: &'a Expr) {
        self.last_lit_effect = false;
        let mut effect = false;
        match &a.kind {
            ExprKind::FuncLit(l) => {
                if let Some(alt) = &l.alt {
                    if !self.is_type_name(&l.ret.name) {
                        self.lit_calls.insert((a.pos.offset, a.end));
                        self.at_place(pt, alt, None);
                        return;
                    }
                }
                let want_ret = match pt {
                    Ty::Func(i) if l.block && self.ftys[i as usize].1.is_empty() => {
                        Some(self.ftys[i as usize].0)
                    }
                    _ => None,
                };
                if l.block && want_ret.is_none() {
                    let msg = format!(
                        "`{callee}` cannot take a block: its last parameter has type {}",
                        self.tn(pt)
                    );
                    let hint = matches!(pt, Ty::Func(_))
                        .then(|| "a block has no parameters; pass a literal instead".to_string());
                    self.err("R26", a.pos, msg, hint);
                    return;
                }
                let t = self.lit_body(a, l, want_ret);
                self.expect(pt, t, a);
                effect = self.last_lit_effect;
            }
            ExprKind::Ident(n) if matches!(pt, Ty::Func(_)) => {
                let ty = if let Some(t) = self.local(n).map(|l| l.ty) {
                    self.capture(n, t);
                    matches!(t, Ty::Func(_)).then_some(t)
                } else if let Some(&id) = self.funcs.get(n.as_str()) {
                    effect = self.derived.as_ref().is_some_and(|d| d[id].effect);
                    if let Some(cur) = self.cur {
                        self.facts[cur].calls.push(id);
                    }
                    let ps = self.fns[id]
                        .params
                        .iter()
                        .map(|p| (p.ty, p.is_mut))
                        .collect();
                    let r = self.fns[id].ret;
                    Some(self.func_ty(r, ps))
                } else {
                    None
                };
                match ty {
                    Some(t) => {
                        self.exprs.insert((a.pos.offset, a.end), t);
                        self.expect(pt, t, a);
                    }
                    None => {
                        self.at_place(pt, a, None);
                    }
                }
            }
            ExprKind::Field(base, m)
                if matches!(pt, Ty::Func(_)) && self.enum_of(base).is_some() =>
            {
                let eid = self.enum_of(base).unwrap_or(0);
                let fields = match &self.types[eid as usize].kind {
                    Kind::Enum(vs) => vs.iter().find(|v| v.name == m).map(|v| v.fields.clone()),
                    Kind::Struct(_) => None,
                };
                match fields.filter(|f| !f.is_empty()) {
                    Some(fs) => {
                        let ps = fs.iter().map(|f| (f.0, false)).collect();
                        let t = self.func_ty(Ty::Named(eid), ps);
                        self.exprs.insert((a.pos.offset, a.end), t);
                        self.expect(pt, t, a);
                    }
                    None => {
                        self.at_place(pt, a, None);
                    }
                }
            }
            _ => {
                self.at_place(pt, a, None);
            }
        }
        if effect {
            self.call_lit.insert((call.pos.offset, call.end));
        }
    }

    /// The body of a literal (`block_ret` is the return type of a block): its function type.
    fn lit_body(&mut self, e: &'a Expr, l: &'a FuncLit, block_ret: Option<Ty>) -> Ty {
        let ret = match block_ret {
            Some(r) => r,
            None => self.ty(&l.ret, true),
        };
        let mut ps = Vec::new();
        for p in &l.params {
            ps.push((self.ty(&p.ty, false), p.mut_pos.is_some()));
        }
        let fty = self.func_ty(ret, ps.clone());
        self.frames.push(LitFrame {
            ret,
            base: self.scopes.len(),
            captured: Vec::new(),
            declared: Vec::new(),
            changed: Vec::new(),
            effect: false,
        });
        let ctx = std::mem::take(&mut self.ctx);
        let errors_before = self.diags.len();
        self.scopes.push(Vec::new());
        for (p, (t, _)) in l.params.iter().zip(&ps) {
            self.declare(&p.name, p.pos, *t, false, Val::Runtime, None);
        }
        self.block(&l.body);
        self.scopes.pop();
        self.ctx = ctx;
        if !self.no_value(ret) && ret != Ty::Err && !terminates(&l.body, &self.complete) {
            let msg = "the literal does not return a value on every path";
            let hint = format!("end it with return, e.g. return {}", self.zero(ret));
            self.err("R8", e.pos, msg, Some(hint));
        }
        let Some(fr) = self.frames.pop() else {
            return fty;
        };
        if self.derived.is_some() && self.diags.len() == errors_before {
            let escape: Vec<String> = (l.params.iter())
                .filter(|p| p.mut_pos.is_some())
                .map(|p| p.name.clone())
                .collect();
            for (place, pos) in live::lost(&l.body, &self.call_changes, &escape) {
                let root = place.split('.').next().unwrap_or("").to_string();
                if !fr.declared.contains(&root) {
                    continue; // a captured name: reported as a lost change already
                }
                if l.params.iter().any(|p| p.name == root) {
                    let msg = format!("the change to `{place}` is lost: `{root}` is a copy");
                    self.err("R6", pos, msg, None);
                } else {
                    let msg = format!("`{place}` is changed but never read afterwards");
                    let hint = (self.lost_hints.get(&pos.offset).cloned())
                        .unwrap_or_else(|| "use the value, or remove the change".to_string());
                    self.err("R6", pos, msg, Some(hint));
                }
            }
        }
        self.last_lit_effect = fr.effect;
        if fr.effect {
            if let Some(outer) = self.frames.last_mut() {
                outer.effect = true;
            }
        }
        let captures = fr.captured;
        self.lits
            .insert((e.pos.offset, e.end), LitInfo { ty: fty, captures });
        self.exprs.insert((e.pos.offset, e.end), fty);
        fty
    }
}

impl<'a> Checker<'a> {
    /// A call right before the `{` of a header that lacks only a block (R26): say where the
    /// parentheses go.
    pub(super) fn header_block_hint(
        &self,
        e: &Expr,
        params: &[(Ty, bool, String)],
        found: usize,
    ) -> Option<String> {
        let last = params.last()?.0;
        let block = matches!(last, Ty::Func(i) if self.ftys[i as usize].1.is_empty());
        let rest = self.src.get(e.end as usize..)?;
        let line_end = rest.find('\n').unwrap_or(rest.len());
        if !block || found + 1 != params.len() || !rest.trim_start().starts_with('{') {
            return None;
        }
        let before = &self.src[..e.pos.offset as usize];
        let from = before.rfind('\n').map_or(0, |i| i + 1);
        Some(format!(
            "a block in a header needs parentheses around the call: {}({} {{ … }}){}",
            before[from..].trim_start(),
            self.text(e),
            &rest[..line_end]
        ))
    }
}
