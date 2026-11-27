//! Function values (R26): `{fn, env}`. A literal is a C function of its own that takes its
//! environment, a struct of the values it captured, as first parameter; a named function or a
//! variant constructor goes through a wrapper with `env = NULL`. The environment lives in a
//! temporary of the function that evaluates the literal and ends with the call.

use std::fmt::Write;

use super::*;

/// What belongs to the C function being written, set aside while a literal is written.
struct FnState<'a> {
    temps: Vec<(String, String)>,
    mtemps: Vec<(Ty, String)>,
    bufs: Vec<String>,
    scopes: Vec<Vec<(Ty, String)>>,
    loops: Vec<usize>,
    used: Vec<(Ty, String)>,
    out: String,
    derefs: HashSet<&'a str>,
    lows: HashMap<&'a str, i64>,
    steady: HashSet<&'a str>,
    slots: Vec<String>,
    idx_temps: HashMap<(u32, u32), String>,
    ret: Ty,
    in_test: bool,
    nested: usize,
    caps: HashMap<String, String>,
}

impl<'a> Emitter<'a> {
    fn save_fn(&mut self) -> FnState<'a> {
        FnState {
            temps: std::mem::take(&mut self.temps),
            mtemps: std::mem::take(&mut self.mtemps),
            bufs: std::mem::take(&mut self.bufs),
            scopes: std::mem::take(&mut self.scopes),
            loops: std::mem::take(&mut self.loops),
            used: std::mem::take(&mut self.used),
            out: std::mem::take(&mut self.out),
            derefs: std::mem::take(&mut self.derefs),
            lows: std::mem::take(&mut self.lows),
            steady: std::mem::take(&mut self.steady),
            slots: std::mem::take(&mut self.slots),
            idx_temps: std::mem::take(&mut self.idx_temps),
            ret: self.ret,
            in_test: self.in_test,
            nested: std::mem::take(&mut self.nested),
            caps: std::mem::take(&mut self.caps),
        }
    }

    fn restore_fn(&mut self, s: FnState<'a>) {
        self.temps = s.temps;
        self.mtemps = s.mtemps;
        self.bufs = s.bufs;
        self.scopes = s.scopes;
        self.loops = s.loops;
        self.used = s.used;
        self.out = s.out;
        self.derefs = s.derefs;
        self.lows = s.lows;
        self.steady = s.steady;
        self.slots = s.slots;
        self.idx_temps = s.idx_temps;
        self.ret = s.ret;
        self.in_test = s.in_test;
        self.nested = s.nested;
        self.caps = s.caps;
    }

    /// The C place of a name of the current function: a captured value of a literal, a
    /// pointer (`mut` parameter, `self` that is changed) or a plain local.
    pub(super) fn name_ref(&self, n: &str) -> String {
        if let Some(c) = self.caps.get(n) {
            return c.clone();
        }
        let d = if n == "self" {
            "d_self".to_string()
        } else {
            format!("d_{n}")
        };
        if self.derefs.contains(n) {
            format!("(*{d})")
        } else {
            d
        }
    }

    /// The C parameters of a function type, without the environment.
    fn fn_params(&self, ps: &[(Ty, bool)]) -> Vec<String> {
        ps.iter()
            .map(|(t, m)| format!("{}{}", self.c_ty(*t), if *m { " *" } else { "" }))
            .collect()
    }

    /// `typedef` of every function type: a function pointer and the environment.
    pub(super) fn fn_type_defs(&self) -> String {
        let mut c = String::new();
        for (i, (r, ps)) in self.info.ftys.iter().enumerate() {
            let mut p = vec!["void *".to_string()];
            p.extend(self.fn_params(ps));
            let _ = writeln!(
                c,
                "typedef struct {{ {} (*fn)({}); dif_env *env; }} dfn_{i};",
                self.c_ty(*r),
                p.join(", ")
            );
        }
        c
    }

    /// The environment of a function without captured values: shared, never freed.
    fn static_env(&mut self, name: &str, text: &str) -> String {
        let env = format!("dsenv_{name}");
        let _ = writeln!(
            self.lit_protos,
            "static dif_env {env} = {{0, {}, NULL, NULL, NULL}};",
            super::c_string(text.as_bytes())
        );
        format!("&{env}")
    }

    /// A literal or a block as a value `{fn, env}`: the captured values are copied into a
    /// counted environment here, in the order of the captures (R26).
    pub(super) fn func_lit(&mut self, e: &'a Expr, l: &'a FuncLit) -> String {
        let span = (e.pos.offset, e.end);
        if let (Some(alt), true) = (&l.alt, self.info.lit_calls.contains(&span)) {
            return self.expr(alt);
        }
        let info = &self.info.lits[&span];
        let (fty, caps) = (info.ty, info.captures.clone());
        let id = self.uid;
        self.uid += 1;
        let name = format!("dl{id}");
        let text = format!("fn@{}:{}:{}", self.file_raw, e.pos.line, e.pos.col);
        self.lit_fn(&name, l, fty, &caps);
        let ct = self.c_ty(fty);
        if caps.is_empty() {
            let env = self.static_env(&name, &text);
            return format!("(({ct}){{{name}, {env}}})");
        }
        self.env_helpers(&name, &text, &caps);
        let mut fill = String::new();
        for (n, ty) in &caps {
            let src = self.name_ref(n);
            let v = self.copy(*ty, &src);
            let _ = write!(fill, "en{id}->f_{n} = {v}; ");
        }
        format!(
            "({{ struct {name}_env *en{id} = dif_alloc(sizeof *en{id}); en{id}->h = (dif_env){{1, {}, {name}_eq, {name}_hash, {name}_drop}}; {fill}({ct}){{{name}, &en{id}->h}}; }})",
            super::c_string(text.as_bytes())
        )
    }

    /// Release, equality and hash of the captured values of a literal (R28).
    fn env_helpers(&mut self, name: &str, _text: &str, caps: &[(String, Ty)]) {
        let st = format!("struct {name}_env");
        let mut drop = String::new();
        let mut eq = Vec::new();
        let mut hash = String::new();
        for (n, ty) in caps {
            if self.managed(*ty) {
                let _ = write!(drop, " {};", self.drop_e(*ty, &format!("s->f_{n}")));
            }
            eq.push(self.eq(*ty, &format!("x->f_{n}"), &format!("y->f_{n}")));
            let d = self.dty(*ty);
            let _ = write!(hash, " h = dif_hash_mix(h, {d}.hash(&{d}, &x->f_{n}));");
        }
        let protos = format!(
            "static void {name}_drop(dif_env *e_);\nstatic bool {name}_eq(const dif_env *a, const dif_env *b);\nstatic uint64_t {name}_hash(const dif_env *e_);\n"
        );
        self.lit_protos.push_str(&protos);
        let _ = writeln!(
            self.lit_defs,
            "static void {name}_drop(dif_env *e_) {{ {st} *s = ({st} *)e_; (void)s;{drop} }}\nstatic bool {name}_eq(const dif_env *a, const dif_env *b) {{ const {st} *x = (const {st} *)a; const {st} *y = (const {st} *)b; (void)x; (void)y; return {}; }}\nstatic uint64_t {name}_hash(const dif_env *e_) {{ const {st} *x = (const {st} *)e_; uint64_t h = 5; (void)x;{hash} return h; }}",
            eq.join(" && ")
        );
    }

    /// The C function of a literal, and the struct of its environment.
    fn lit_fn(&mut self, name: &str, l: &'a FuncLit, fty: Ty, caps: &[(String, Ty)]) {
        let Ty::Func(fi) = fty else { return };
        let (ret, ps) = self.info.ftys[fi as usize].clone();
        if !caps.is_empty() {
            let fields: String = caps
                .iter()
                .map(|(n, t)| format!(" {} f_{n};", self.c_ty(*t)))
                .collect();
            let _ = writeln!(
                self.lit_protos,
                "struct {name}_env {{ dif_env h;{fields} }};"
            );
        }
        let in_test = self.in_test;
        let saved = self.save_fn();
        self.start_fn(ret, in_test);
        for (n, _) in caps {
            self.caps.insert(n.clone(), format!("e_->f_{n}"));
        }
        let mut params = vec!["void *env_".to_string()];
        for (p, (t, m)) in l.params.iter().zip(&ps) {
            let star = if *m { " *" } else { " " };
            params.push(format!("{}{star}d_{}", self.c_ty(*t), p.name));
            if *m {
                self.derefs.insert(&p.name);
            } else {
                self.own_local(*t, format!("d_{}", p.name));
            }
            self.line(1, &format!("(void)d_{};", p.name));
        }
        self.block_body(&l.body, 1);
        let header = format!("static {} {name}({})", self.c_ty(ret), params.join(", "));
        let tail = self.fn_tail(&l.body, ret != Ty::Void);
        let mut c = format!("{header} {{\n");
        if caps.is_empty() {
            c.push_str("    (void)env_;\n");
        } else {
            let _ = writeln!(c, "    struct {name}_env *e_ = env_;\n    (void)e_;");
        }
        c.push_str(&self.temp_decls());
        c.push_str(&self.out);
        c.push_str(&tail);
        c.push_str("}\n");
        self.restore_fn(saved);
        let _ = writeln!(self.lit_protos, "{header};");
        self.lit_defs.push_str(&c);
    }

    /// A named function as a value: through a wrapper that ignores the environment.
    pub(super) fn fn_value(&mut self, f: &'a Func, fty: Ty) -> String {
        let name = format!("dw_{}", f.name);
        if self.wrappers.insert(name.clone()) {
            let Ty::Func(fi) = fty else {
                return String::new();
            };
            let (ret, ps) = self.info.ftys[fi as usize].clone();
            let names: Vec<String> = (0..ps.len()).map(|k| format!("a{k}")).collect();
            let params: Vec<String> = ps
                .iter()
                .zip(&names)
                .map(|((t, m), n)| format!(", {}{}{n}", self.c_ty(*t), if *m { " *" } else { " " }))
                .collect();
            let call = format!("{}({})", Self::cname(f), names.join(", "));
            let body = if ret == Ty::Void {
                format!("{call};")
            } else {
                format!("return {call};")
            };
            let header = format!(
                "static {} {name}(void *e_{})",
                self.c_ty(ret),
                params.concat()
            );
            let _ = writeln!(self.lit_protos, "{header};");
            let _ = writeln!(self.lit_defs, "{header} {{ (void)e_; {body} }}");
            self.static_env(&name, &f.name);
        }
        let env = format!("&dsenv_{name}");
        format!("(({}){{{name}, {env}}})", self.c_ty(fty))
    }

    /// A variant constructor `E.V` as a value.
    pub(super) fn ctor_value(&mut self, eid: u32, variant: &str, fty: Ty) -> String {
        let name = format!("dwv_{}_{variant}", types::cid(self.tname(eid)));
        if self.wrappers.insert(name.clone()) {
            let k = self.variant_index(eid, variant);
            let fields = self.variant_fields(eid, k);
            let params: String = fields
                .iter()
                .enumerate()
                .map(|(j, t)| format!(", {} f{j}", self.c_ty(*t)))
                .collect();
            let sets: String = (0..fields.len())
                .map(|j| format!(" r.u.v{k}.f{j} = f{j};"))
                .collect();
            let ct = self.c_ty(Ty::Named(eid));
            let header = format!("static {ct} {name}(void *e_{params})");
            let shown = self.tname(eid).rsplit('.').next().unwrap_or_default();
            self.static_env(&name, &format!("{shown}.{variant}"));
            let _ = writeln!(self.lit_protos, "{header};");
            let _ = writeln!(
                self.lit_defs,
                "{header} {{ (void)e_; {ct} r = {{0}}; r.tag = {k};{sets} return r; }}"
            );
        }
        let env = format!("&dsenv_{name}");
        format!("(({}){{{name}, {env}}})", self.c_ty(fty))
    }

    /// `f(args)` for a function value `f`: a parameter or captured value, a field, an
    /// element. Anything but a name is copied into a temporary first, so that the call
    /// keeps the code alive however its arguments change the place.
    pub(super) fn call_value(
        &mut self,
        e: &'a Expr,
        f: &'a Expr,
        fi: u32,
        args: &'a [Expr],
    ) -> String {
        let ps = self.info.ftys[fi as usize].1.clone();
        let items: Vec<Arg<'a>> = args
            .iter()
            .zip(&ps)
            .map(|(a, (t, m))| match (&a.kind, m) {
                (ExprKind::Mut(x), true) => Arg::Addr(x),
                _ => Arg::Val(a, *t),
            })
            .collect();
        let (pre, fv) = match &f.kind {
            ExprKind::Ident(n) => (String::new(), self.name_ref(n)),
            _ => {
                let t = self.ty(f);
                let v = self.bor(f);
                let m = self.mtemp(t);
                let pre = format!("{}, {m} = {}, ", self.drop_e(t, &m), self.copy(t, &v));
                (pre, m)
            }
        };
        let call = self.apply_with(
            &format!("{fv}.fn"),
            Some(format!("{fv}.env")),
            &items,
            (e.pos.line as usize, e.pos.col as usize),
        );
        Self::with_pre(pre, call)
    }

    /// The finished program: the declarations of literals and wrappers where the marker
    /// stands, their definitions at the end.
    pub(super) fn finish(&self, c: String) -> String {
        c.replacen(LITS_MARK, &self.lit_protos, 1) + &self.lit_defs
    }
}
