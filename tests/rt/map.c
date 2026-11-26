// Maps: insertion order, copy-on-write, removal, equality, text form, collisions.
#include "t.h"

DIF_CONST dif_ty ty_msi = DIF_MAP_TY(&dif_ty_str, &dif_ty_int);
DIF_CONST dif_ty ty_mss = DIF_MAP_TY(&dif_ty_str, &dif_ty_str);
DIF_CONST dif_ty ty_ls = DIF_LIST_TY(&dif_ty_str);
DIF_CONST dif_ty ty_msls = DIF_MAP_TY(&dif_ty_str, &ty_ls);
#define KT (&dif_ty_str)
#define VT (&dif_ty_int)

static void set(dif_map **m, const char *k, int64_t v) {
    dif_str key = dif_cstr(k);
    dif_map_set(KT, VT, m, &key, &v, 1, 1);
}
static const int64_t *get(const dif_map *m, const char *k) {
    dif_str key = DIF_S("");
    key.p = k;
    key.n = (int64_t)strlen(k);
    return dif_map_get(KT, VT, m, &key, 1, 1);
}
static bool rem(dif_map **m, const char *k) {
    dif_str key = {k, (int64_t)strlen(k), NULL};
    return dif_map_remove(KT, VT, m, &key, 1, 1);
}
static void check_text(const dif_map *m, const char *want, int line) {
    dif_buf b = dif_buf_new();
    dif_map_put(KT, VT, &b, m);
    dif_str s = dif_take(&b);
    t_count++;
    if (s.n != (int64_t)strlen(want) || memcmp(s.p, want, (size_t)s.n) != 0) {
        fprintf(stderr, "map.c:%d: text form \"%.*s\", expected \"%s\"\n", line, (int)s.n, s.p, want);
        t_fail++;
    }
    dif_str_dec(s);
}
#define EXPECT_TEXT(m, want) check_text(m, want, __LINE__)

static void test_basic(void) {
    dif_map *m = NULL;
    CHECK(dif_map_len(m) == 0 && get(m, "x") == NULL);
    EXPECT_TEXT(m, "{}");
    // the word count of the spec (R30)
    const char *words[] = {"to", "be", "or", "not", "to", "be"};
    for (int i = 0; i < 6; i++) {
        const int64_t *c = get(m, words[i]);
        set(&m, words[i], (c ? *c : 0) + 1);
    }
    EXPECT_TEXT(m, "{\"to\": 2, \"be\": 2, \"or\": 1, \"not\": 1}");
    CHECK(*get(m, "be") == 2 && get(m, "xx") == NULL);
    dif_str k = DIF_S("or");
    CHECK(dif_map_has(KT, VT, m, &k, 1, 1));
    CHECK(rem(&m, "or") && !rem(&m, "or") && !rem(&m, "nothing"));
    dif_list *keys = dif_map_keys(KT, VT, m);
    dif_buf b = dif_buf_new();
    dif_list_put(KT, &b, keys);
    CHECK_BUF(b, "[\"to\", \"be\", \"not\"]");
    dif_list_dec(KT, keys);
    dif_list *vals = dif_map_values(KT, VT, m);
    dif_list_put(VT, &b, vals);
    CHECK_BUF(b, "[2, 2, 1]");
    dif_list_dec(VT, vals);
    CHECK(dif_map_len(m) == 3);
    // replacing keeps the place; remove and set again goes to the end
    set(&m, "to", 9);
    EXPECT_TEXT(m, "{\"to\": 9, \"be\": 2, \"not\": 1}");
    rem(&m, "to");
    set(&m, "to", 10);
    EXPECT_TEXT(m, "{\"be\": 2, \"not\": 1, \"to\": 10}");
    dif_map_dec(KT, VT, m);
    // an empty map after removing everything
    m = NULL;
    set(&m, "a", 1);
    rem(&m, "a");
    EXPECT_TEXT(m, "{}");
    CHECK(dif_map_len(m) == 0 && get(m, "a") == NULL && dif_map_keys(KT, VT, m) == NULL);
    set(&m, "b", 2);
    EXPECT_TEXT(m, "{\"b\": 2}");
    dif_map_dec(KT, VT, m);
}

static void test_cow(void) {
    dif_map *a = NULL;
    set(&a, "x", 1);
    set(&a, "y", 2);
    dif_map *b = dif_map_inc(a); // map b = a
    CHECK(a->rc == 2);
    set(&b, "x", 10);
    set(&b, "z", 3);
    EXPECT_TEXT(a, "{\"x\": 1, \"y\": 2}");
    EXPECT_TEXT(b, "{\"x\": 10, \"y\": 2, \"z\": 3}");
    CHECK(a->rc == 1 && b->rc == 1);
    dif_map *c = dif_map_inc(b);
    CHECK(rem(&c, "y"));
    EXPECT_TEXT(b, "{\"x\": 10, \"y\": 2, \"z\": 3}");
    EXPECT_TEXT(c, "{\"x\": 10, \"z\": 3}");
    CHECK(!rem(&c, "nope") && c->rc == 1); // no copy when nothing changes
    // iteration over a snapshot while the variable changes
    dif_map *snap = dif_map_inc(b);
    int64_t seen = 0;
    for (int64_t i = 0; i < dif_map_cnt(snap); i++) {
        const void *k = dif_map_key_at(KT, VT, snap, i);
        if (!k) continue;
        seen++;
        set(&b, "more", seen);
    }
    CHECK(seen == 3 && dif_map_len(b) == 4 && dif_map_len(snap) == 3);
    dif_map_dec(KT, VT, snap);
    dif_map_dec(KT, VT, a);
    dif_map_dec(KT, VT, b);
    dif_map_dec(KT, VT, c);
}

static void test_eq(void) {
    dif_map *a = NULL, *b = NULL;
    CHECK(dif_map_eq(KT, VT, a, b));
    set(&a, "x", 1);
    set(&a, "y", 2);
    CHECK(!dif_map_eq(KT, VT, a, b));
    set(&b, "y", 2);
    set(&b, "x", 1); // other order
    CHECK(dif_map_eq(KT, VT, a, b));
    set(&b, "x", 5);
    CHECK(!dif_map_eq(KT, VT, a, b));
    set(&b, "x", 1);
    set(&b, "z", 1);
    CHECK(!dif_map_eq(KT, VT, a, b));
    rem(&b, "z");
    CHECK(dif_map_eq(KT, VT, a, b));
    CHECK(dif_map_hash_t(&ty_msi, &a) == dif_map_hash_t(&ty_msi, &b));
    CHECK(ty_msi.eq(&ty_msi, &a, &b));
    dif_map_dec(KT, VT, a);
    dif_map_dec(KT, VT, b);
}

static void test_many(void) {
    dif_map *m = NULL;
    char key[32];
    for (int64_t i = 0; i < 20000; i++) {
        snprintf(key, sizeof key, "key%" PRId64, i);
        set(&m, key, i);
    }
    CHECK(dif_map_len(m) == 20000);
    int ok = 1;
    for (int64_t i = 0; i < 20000; i += 7) {
        snprintf(key, sizeof key, "key%" PRId64, i);
        const int64_t *v = get(m, key);
        ok &= v && *v == i;
    }
    CHECK(ok);
    for (int64_t i = 0; i < 20000; i++) {
        if (i % 3 == 0) continue;
        snprintf(key, sizeof key, "key%" PRId64, i);
        rem(&m, key);
    }
    CHECK(dif_map_len(m) == 6667);
    // many insert/remove cycles must not grow the entry array
    for (int64_t i = 0; i < 100000; i++) {
        set(&m, "tmp", i);
        rem(&m, "tmp");
    }
    CHECK(m->cap < 100000 && dif_map_len(m) == 6667);
    dif_list *keys = dif_map_keys(KT, VT, m);
    CHECK(dif_list_len(keys) == 6667 && dif_str_eq_t(KT, &((dif_str *)dif_list_data(keys))[1], &(dif_str){"key3", 4, NULL}));
    dif_list_dec(KT, keys);
    dif_map_dec(KT, VT, m);
}

// keys whose hash always collides, and a key type with a counted resource
typedef struct { int64_t id; dif_str tag; } ckey;
static void ck_inc(const dif_ty *t, const void *p) { (void)t; dif_str_inc(((const ckey *)p)->tag); }
static void ck_dec(const dif_ty *t, void *p) { (void)t; dif_str_dec(((ckey *)p)->tag); }
static bool ck_eq(const dif_ty *t, const void *a, const void *b) { (void)t; return ((const ckey *)a)->id == ((const ckey *)b)->id; }
static void ck_put(const dif_ty *t, dif_buf *b, const void *p) { (void)t; dif_put_i64(b, ((const ckey *)p)->id); }
static uint64_t ck_hash(const dif_ty *t, const void *p) { (void)t; (void)p; return 1; }
DIF_CONST dif_ty ty_ck = {sizeof(ckey), NULL, NULL, 0, ck_inc, ck_dec, ck_eq, ck_put, NULL, ck_hash};

static void test_collisions_and_resources(void) {
    dif_map *m = NULL;
    for (int64_t i = 0; i < 50; i++) {
        ckey k = {i, dif_cstr("tag")};
        dif_str v = dif_cstr("value");
        dif_map_set(&ty_ck, &dif_ty_str, &m, &k, &v, 1, 1);
    }
    ckey q = {37, DIF_S("")};
    const dif_str *v = dif_map_get(&ty_ck, &dif_ty_str, m, &q, 1, 1);
    CHECK(v && dif_eq(*v, DIF_S("value")));
    q.id = 99;
    CHECK(dif_map_get(&ty_ck, &dif_ty_str, m, &q, 1, 1) == NULL);
    // setting an existing key drops the passed key and the old value
    ckey k2 = {5, dif_cstr("other")};
    dif_str v2 = dif_cstr("new");
    dif_map_set(&ty_ck, &dif_ty_str, &m, &k2, &v2, 1, 1);
    q.id = 5;
    CHECK(dif_eq(*(const dif_str *)dif_map_get(&ty_ck, &dif_ty_str, m, &q, 1, 1), DIF_S("new")));
    dif_map *copy = dif_map_inc(m);
    for (int64_t i = 0; i < 50; i += 2) {
        q.id = i;
        CHECK(dif_map_remove(&ty_ck, &dif_ty_str, &m, &q, 1, 1));
    }
    CHECK(dif_map_len(m) == 25 && dif_map_len(copy) == 50);
    dif_map_dec(&ty_ck, &dif_ty_str, copy);
    dif_map_dec(&ty_ck, &dif_ty_str, m);
}

static void test_nested(void) {
    // map[str][]str: values with their own counters, a map inside a map
    dif_map *m = NULL;
    dif_str k = dif_cstr("fruits");
    dif_list *l = NULL;
    dif_str a = dif_cstr("apple"), b = dif_cstr("pear");
    dif_list_push(&dif_ty_str, &l, &a);
    dif_list_push(&dif_ty_str, &l, &b);
    dif_map_set(&dif_ty_str, &ty_ls, &m, &k, &l, 1, 1);
    dif_map *copy = dif_map_inc(m);
    dif_str key = DIF_S("fruits");
    dif_list **lp = (dif_list **)(uintptr_t)dif_map_get(&dif_ty_str, &ty_ls, m, &key, 1, 1);
    dif_list *mine = dif_list_inc(*lp); // read, change the copy, store it back (R30)
    dif_str c = dif_cstr("fig");
    dif_list_push(&dif_ty_str, &mine, &c);
    dif_str k2 = dif_cstr("fruits");
    dif_map_set(&dif_ty_str, &ty_ls, &m, &k2, &mine, 1, 1);
    dif_buf buf = dif_buf_new();
    ty_msls.put(&ty_msls, &buf, &m);
    CHECK_BUF(buf, "{\"fruits\": [\"apple\", \"pear\", \"fig\"]}");
    ty_msls.put(&ty_msls, &buf, &copy);
    CHECK_BUF(buf, "{\"fruits\": [\"apple\", \"pear\"]}");
    DIF_CONST dif_ty ty_mm = DIF_MAP_TY(&dif_ty_str, &ty_msls);
    dif_map *outer = NULL;
    dif_str ok = dif_cstr("inner");
    dif_map_set(&dif_ty_str, &ty_msls, &outer, &ok, &m, 1, 1);
    ty_mm.put(&ty_mm, &buf, &outer);
    CHECK_BUF(buf, "{\"inner\": {\"fruits\": [\"apple\", \"pear\", \"fig\"]}}");
    dif_map_dec(&dif_ty_str, &ty_msls, outer);
    dif_map_dec(&dif_ty_str, &ty_ls, copy);
}

static void test_float_keys(void) {
    DIF_CONST dif_ty ty_lf = DIF_LIST_TY(&dif_ty_f64);
    dif_map *m = NULL;
    double z = 0.0, nz = -0.0, one = 1.5;
    int64_t v = 1, w = 2;
    dif_map_set(&dif_ty_f64, VT, &m, &z, &v, 1, 1);
    dif_map_set(&dif_ty_f64, VT, &m, &nz, &w, 1, 1); // 0.0 and -0.0 are the same key
    CHECK(dif_map_len(m) == 1 && *(const int64_t *)dif_map_get(&dif_ty_f64, VT, m, &z, 1, 1) == 2);
    dif_map_set(&dif_ty_f64, VT, &m, &one, &v, 1, 1);
    dif_buf b = dif_buf_new();
    dif_map_put(&dif_ty_f64, VT, &b, m);
    CHECK_BUF(b, "{0: 2, 1.5: 1}"); // the first spelling stays
    double nan = NAN;
    EXPECT_PANIC("NaN cannot be a map key (main.dif:3:5)", dif_map_set(&dif_ty_f64, VT, &m, &nan, &v, 3, 5));
    EXPECT_PANIC("NaN cannot be a map key (main.dif:3:5)", dif_map_get(&dif_ty_f64, VT, m, &nan, 3, 5));
    EXPECT_PANIC("NaN cannot be a map key (main.dif:3:5)", dif_map_has(&dif_ty_f64, VT, m, &nan, 3, 5));
    EXPECT_PANIC("NaN cannot be a map key (main.dif:3:5)", dif_map_remove(&dif_ty_f64, VT, &m, &nan, 3, 5));
    // a NaN inside a list key counts too
    dif_list *k = NULL;
    dif_list_push(&dif_ty_f64, &k, &one);
    dif_list_push(&dif_ty_f64, &k, &nan);
    dif_map *lm = NULL;
    EXPECT_PANIC("NaN cannot be a map key (main.dif:4:4)", dif_map_set(&ty_lf, VT, &lm, &k, &v, 4, 4));
    dif_list_dec(&dif_ty_f64, k);
    dif_map_dec(&dif_ty_f64, VT, m);
}

int main(void) {
    test_float_keys();
    test_basic();
    test_cow();
    test_eq();
    test_many();
    test_collisions_and_resources();
    test_nested();
    T_DONE();
}
