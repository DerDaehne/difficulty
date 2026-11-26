// Descriptors, arrays, lists (copy-on-write), str.split/lines/join.
#include "t.h"

DIF_CONST dif_ty ty_li = DIF_LIST_TY(&dif_ty_int);        // [][]int ... element []int
DIF_CONST dif_ty ty_lli = DIF_LIST_TY(&ty_li);            // element [][]int
DIF_CONST dif_ty ty_ls = DIF_LIST_TY(&dif_ty_str);

static dif_list *ints(int n, const int64_t *v) {
    dif_list *l = NULL;
    for (int i = 0; i < n; i++) dif_list_push(&dif_ty_int, &l, &v[i]);
    return l;
}
#define INTS(...) ints((int)(sizeof((int64_t[]){__VA_ARGS__}) / 8), (int64_t[]){__VA_ARGS__})

static void check_ints(const dif_list *l, int n, const int64_t *v, int line) {
    t_count++;
    if (dif_list_len(l) != n) {
        fprintf(stderr, "list.c:%d: length %" PRId64 ", expected %d\n", line, dif_list_len(l), n);
        t_fail++;
        return;
    }
    for (int i = 0; i < n; i++)
        if (((const int64_t *)dif_list_data(l))[i] != v[i]) {
            fprintf(stderr, "list.c:%d: element %d is %" PRId64 ", expected %" PRId64 "\n", line, i,
                    ((const int64_t *)dif_list_data(l))[i], v[i]);
            t_fail++;
            return;
        }
}
#define EXPECT_INTS(l, ...) check_ints(l, (int)(sizeof((int64_t[]){__VA_ARGS__}) / 8), (int64_t[]){__VA_ARGS__}, __LINE__)

static void check_text(const dif_ty *et, const dif_list *l, const char *want, int line) {
    dif_buf b = dif_buf_new();
    dif_list_put(et, &b, l);
    dif_str s = dif_take(&b);
    t_count++;
    if (s.n != (int64_t)strlen(want) || memcmp(s.p, want, (size_t)s.n) != 0) {
        fprintf(stderr, "list.c:%d: text form \"%.*s\", expected \"%s\"\n", line, (int)s.n, s.p, want);
        t_fail++;
    }
    dif_str_dec(s);
}
#define EXPECT_TEXT(et, l, want) check_text(et, l, want, __LINE__)

static void test_push_pop(void) {
    dif_list *l = NULL;
    CHECK(dif_list_len(l) == 0 && dif_list_data(l) == NULL);
    int64_t v;
    CHECK(!dif_list_pop(&dif_ty_int, &l, &v) && l == NULL);
    for (int64_t i = 0; i < 100; i++) dif_list_push(&dif_ty_int, &l, &i);
    CHECK(dif_list_len(l) == 100 && l->rc == 1);
    CHECK(dif_list_pop(&dif_ty_int, &l, &v) && v == 99 && dif_list_len(l) == 99);
    CHECK(*(const int64_t *)dif_list_first(&dif_ty_int, l) == 0);
    CHECK(*(const int64_t *)dif_list_last(&dif_ty_int, l) == 98);
    dif_list_dec(&dif_ty_int, l);
    l = NULL;
    CHECK(dif_list_first(&dif_ty_int, l) == NULL && dif_list_last(&dif_ty_int, l) == NULL);
    uint8_t b = 200;
    dif_list_push(&dif_ty_u8, &l, &b); // another element size
    b = 7;
    dif_list_push(&dif_ty_u8, &l, &b);
    EXPECT_TEXT(&dif_ty_u8, l, "[200, 7]");
    dif_list_dec(&dif_ty_u8, l);
}

static void test_cow(void) {
    dif_list *xs = INTS(1, 2, 3);
    dif_list *ys = dif_list_inc(xs); // []int ys = xs
    CHECK(xs->rc == 2);
    int64_t ten = 10;
    dif_list_set(&dif_ty_int, &xs, 0, &ten, 1, 1); // xs[0] = 10 copies
    CHECK(xs != ys && xs->rc == 1 && ys->rc == 1);
    EXPECT_INTS(xs, 10, 2, 3);
    EXPECT_INTS(ys, 1, 2, 3);
    int64_t four = 4;
    dif_list *snap = dif_list_inc(xs); // snapshot for a loop
    dif_list_push(&dif_ty_int, &xs, &four);
    EXPECT_INTS(snap, 10, 2, 3);
    EXPECT_INTS(xs, 10, 2, 3, 4);
    const void *before = dif_list_data(xs);
    dif_list_push(&dif_ty_int, &xs, &four); // unshared: in place (capacity 4 -> grows, so push again)
    dif_list_push(&dif_ty_int, &xs, &four);
    (void)before;
    CHECK(xs->rc == 1 && dif_list_len(xs) == 6);
    // pop, insert, remove, reverse and sort on a shared list leave the other one alone
    dif_list *zs = dif_list_inc(xs);
    int64_t out;
    CHECK(dif_list_pop(&dif_ty_int, &zs, &out) && out == 4 && dif_list_len(xs) == 6);
    dif_list_dec(&dif_ty_int, zs);
    zs = dif_list_inc(xs);
    dif_list_reverse(&dif_ty_int, &zs);
    EXPECT_INTS(xs, 10, 2, 3, 4, 4, 4);
    EXPECT_INTS(zs, 4, 4, 4, 3, 2, 10);
    dif_list_sort(&dif_ty_int, &zs);
    EXPECT_INTS(zs, 2, 3, 4, 4, 4, 10);
    EXPECT_INTS(xs, 10, 2, 3, 4, 4, 4);
    dif_list_dec(&dif_ty_int, zs);
    zs = dif_list_inc(xs);
    dif_list_insert(&dif_ty_int, &zs, 0, &ten, 1, 1);
    CHECK(dif_list_len(zs) == 7 && dif_list_len(xs) == 6);
    dif_list_dec(&dif_ty_int, zs);
    zs = dif_list_inc(xs);
    dif_list_remove(&dif_ty_int, &zs, 0, &out, 1, 1);
    CHECK(out == 10 && dif_list_len(zs) == 5 && dif_list_len(xs) == 6);
    int64_t *w = dif_list_wat(&dif_ty_int, &zs, 0, 1, 1); // xs[i] += v
    *w += 100;
    CHECK(((const int64_t *)dif_list_data(zs))[0] == 102 && ((const int64_t *)dif_list_data(xs))[1] == 2);
    dif_list_dec(&dif_ty_int, zs);
    dif_list_dec(&dif_ty_int, snap);
    dif_list_dec(&dif_ty_int, ys);
    dif_list_dec(&dif_ty_int, xs);
}

static void test_ops(void) {
    dif_list *xs = INTS(5, 3, 8);
    int64_t one = 1, nine = 9, x;
    dif_list_push(&dif_ty_int, &xs, &one);
    dif_list_insert(&dif_ty_int, &xs, 0, &nine, 1, 1);
    EXPECT_TEXT(&dif_ty_int, xs, "[9, 5, 3, 8, 1]");
    dif_list_insert(&dif_ty_int, &xs, 5, &nine, 1, 1); // at the end is allowed
    dif_list_remove(&dif_ty_int, &xs, 5, &x, 1, 1);
    dif_list_sort(&dif_ty_int, &xs);
    EXPECT_TEXT(&dif_ty_int, xs, "[1, 3, 5, 8, 9]");
    CHECK(dif_list_pop(&dif_ty_int, &xs, &x) && x == 9);
    int64_t eight = 8, seven = 7, three = 3;
    CHECK(dif_list_index_of(&dif_ty_int, xs, &eight) == 3);
    CHECK(dif_list_index_of(&dif_ty_int, xs, &seven) == -1);
    CHECK(dif_list_contains(&dif_ty_int, xs, &three) && !dif_list_contains(&dif_ty_int, xs, &seven));
    CHECK(dif_list_count(&dif_ty_int, xs, &three) == 1);
    dif_list *s = dif_list_slice(&dif_ty_int, xs, 1, 3, 1, 1);
    EXPECT_INTS(s, 3, 5);
    dif_list_dec(&dif_ty_int, s);
    s = dif_list_slice(&dif_ty_int, xs, 2, 2, 1, 1);
    CHECK(s == NULL);
    s = dif_list_slice(&dif_ty_int, xs, 0, 4, 1, 1);
    CHECK(dif_list_eq(&dif_ty_int, s, xs));
    dif_list_set(&dif_ty_int, &s, 0, &seven, 1, 1);
    CHECK(!dif_list_eq(&dif_ty_int, s, xs)); // slice is a copy
    dif_list_dec(&dif_ty_int, s);
    dif_list_reverse(&dif_ty_int, &xs);
    EXPECT_INTS(xs, 8, 5, 3, 1);
    dif_list_dec(&dif_ty_int, xs);
    // empty list
    dif_list *e = NULL;
    CHECK(dif_list_eq(&dif_ty_int, e, NULL));
    EXPECT_TEXT(&dif_ty_int, e, "[]");
    dif_list_sort(&dif_ty_int, &e);
    dif_list_reverse(&dif_ty_int, &e);
    CHECK(e == NULL && dif_list_count(&dif_ty_int, e, &one) == 0 && dif_list_index_of(&dif_ty_int, e, &one) == -1);
    dif_list *r = dif_list_repeat(&dif_ty_int, &seven, 3, 1, 1);
    EXPECT_INTS(r, 7, 7, 7);
    dif_list_dec(&dif_ty_int, r);
    CHECK(dif_list_repeat(&dif_ty_int, &seven, 0, 1, 1) == NULL);
    dif_list *lit = dif_list_from(&dif_ty_int, (int64_t[]){4, 5}, 2);
    EXPECT_INTS(lit, 4, 5);
    dif_list_dec(&dif_ty_int, lit);
    CHECK(dif_list_from(&dif_ty_int, NULL, 0) == NULL);
}

static void test_panics(void) {
    dif_list *xs = INTS(1, 2, 3);
    int64_t x = 0, v = 5;
    EXPECT_PANIC("index 3 out of bounds for length 3 (main.dif:4:13)", dif_list_at(&dif_ty_int, xs, 3, 4, 13));
    EXPECT_PANIC("index -1 out of bounds for length 3 (main.dif:1:1)", dif_list_at(&dif_ty_int, xs, -1, 1, 1));
    EXPECT_PANIC("index 0 out of bounds for length 0 (main.dif:1:1)", dif_list_at(&dif_ty_int, NULL, 0, 1, 1));
    EXPECT_PANIC("index 3 out of bounds for length 3 (main.dif:2:2)", dif_list_set(&dif_ty_int, &xs, 3, &v, 2, 2));
    EXPECT_PANIC("index 3 out of bounds for length 3 (main.dif:2:2)", dif_list_wat(&dif_ty_int, &xs, 3, 2, 2));
    EXPECT_PANIC("index 3 out of bounds for length 3 (main.dif:2:2)", dif_list_remove(&dif_ty_int, &xs, 3, &x, 2, 2));
    EXPECT_PANIC("index 0 out of bounds for length 0 (main.dif:2:2)",
                 dif_list *e = NULL; dif_list_remove(&dif_ty_int, &e, 0, &x, 2, 2));
    EXPECT_PANIC("insert index 4 out of bounds for length 3 (main.dif:2:2)", dif_list_insert(&dif_ty_int, &xs, 4, &v, 2, 2));
    EXPECT_PANIC("insert index -1 out of bounds for length 3 (main.dif:2:2)", dif_list_insert(&dif_ty_int, &xs, -1, &v, 2, 2));
    EXPECT_PANIC("slice 2..4 out of bounds for length 3 (main.dif:3:3)", dif_list_slice(&dif_ty_int, xs, 2, 4, 3, 3));
    EXPECT_PANIC("slice 2..1 out of bounds for length 3 (main.dif:3:3)", dif_list_slice(&dif_ty_int, xs, 2, 1, 3, 3));
    EXPECT_PANIC("slice -1..1 out of bounds for length 3 (main.dif:3:3)", dif_list_slice(&dif_ty_int, xs, -1, 1, 3, 3));
    EXPECT_PANIC("repeat count -2 is negative (main.dif:5:5)", dif_list_repeat(&dif_ty_int, &v, -2, 5, 5));
    EXPECT_PANIC("index 3 out of bounds for length 3 (main.dif:6:6)", int a[3] = {0}; (void)a[dif_idx(3, 3, 6, 6)]);
    CHECK(dif_idx(2, 3, 1, 1) == 2);
    dif_list_dec(&dif_ty_int, xs);
}

// a struct type like the backend would generate: {int key; str name}, ordered by key only
typedef struct { int64_t key; dif_str name; } pair;
static void pair_inc(const dif_ty *t, const void *p) { (void)t; dif_str_inc(((const pair *)p)->name); }
static void pair_dec(const dif_ty *t, void *p) { (void)t; dif_str_dec(((pair *)p)->name); }
static bool pair_eq(const dif_ty *t, const void *a, const void *b) {
    (void)t;
    return ((const pair *)a)->key == ((const pair *)b)->key && dif_eq(((const pair *)a)->name, ((const pair *)b)->name);
}
static int pair_cmp(const dif_ty *t, const void *a, const void *b) {
    (void)t;
    return (((const pair *)a)->key > ((const pair *)b)->key) - (((const pair *)a)->key < ((const pair *)b)->key);
}
static void pair_put(const dif_ty *t, dif_buf *b, const void *p) {
    (void)t;
    dif_put_c(b, "pair{");
    dif_put_i64(b, ((const pair *)p)->key);
    dif_put_c(b, ", ");
    dif_put_quoted(b, ((const pair *)p)->name);
    dif_put_c(b, "}");
}
DIF_CONST dif_ty ty_pair = {sizeof(pair), NULL, NULL, 0, pair_inc, pair_dec, pair_eq, pair_put, pair_cmp, NULL};

static pair mk(int64_t k, const char *name) { return (pair){k, dif_cstr(name)}; }

static void test_struct_elements(void) {
    dif_list *ps = NULL;
    pair a = mk(2, "b1"), b = mk(1, "a1"), c = mk(2, "b2"), d = mk(1, "a2"), e = mk(0, "z");
    dif_list_push(&ty_pair, &ps, &a);
    dif_list_push(&ty_pair, &ps, &b);
    dif_list_push(&ty_pair, &ps, &c);
    dif_list_push(&ty_pair, &ps, &d);
    dif_list_push(&ty_pair, &ps, &e);
    dif_list *copy = dif_list_inc(ps);
    dif_list_sort(&ty_pair, &ps); // stable: equal keys keep their order
    EXPECT_TEXT(&ty_pair, ps, "[pair{0, \"z\"}, pair{1, \"a1\"}, pair{1, \"a2\"}, pair{2, \"b1\"}, pair{2, \"b2\"}]");
    EXPECT_TEXT(&ty_pair, copy, "[pair{2, \"b1\"}, pair{1, \"a1\"}, pair{2, \"b2\"}, pair{1, \"a2\"}, pair{0, \"z\"}]");
    // the copy made by COW shares the texts: counters were raised
    CHECK(((const pair *)dif_list_data(ps))[0].name.rc[0] == 2);
    pair f = mk(1, "a1");
    CHECK(dif_list_contains(&ty_pair, ps, &f));
    pair g = mk(1, "zz");
    CHECK(!dif_list_contains(&ty_pair, ps, &g));
    dif_str_dec(f.name);
    dif_str_dec(g.name);
    pair out;
    CHECK(dif_list_pop(&ty_pair, &ps, &out));
    dif_str_dec(out.name);
    dif_list_dec(&ty_pair, ps);
    dif_list_dec(&ty_pair, copy);
}

static void test_nested(void) {
    dif_list *a = INTS(1, 2);
    dif_list *rows = NULL;
    dif_list_push(&ty_li, &rows, &a);
    dif_list *a2 = dif_list_inc(a);
    dif_list_push(&ty_li, &rows, &a2); // the same inner list twice
    CHECK(a->rc == 2);
    EXPECT_TEXT(&ty_li, rows, "[[1, 2], [1, 2]]");
    // root.kids[0].value = 10: write access to the outer list, then to the inner one
    dif_list *copy = dif_list_inc(rows); // []int rows2 = rows
    dif_list **inner = dif_list_wat(&ty_li, &rows, 0, 1, 1);
    int64_t *el = dif_list_wat(&dif_ty_int, inner, 0, 1, 1);
    *el = 10;
    EXPECT_TEXT(&ty_li, rows, "[[10, 2], [1, 2]]");
    EXPECT_TEXT(&ty_li, copy, "[[1, 2], [1, 2]]");
    dif_list *rows2 = NULL;
    dif_list_push(&ty_lli, &rows2, &rows);
    dif_list *r3 = dif_list_inc(rows2);
    CHECK(dif_list_eq_t(&ty_lli, &rows2, &r3));
    dif_list_dec(&ty_lli, r3);
    dif_list_dec(&ty_lli, rows2);
    dif_list_dec(&ty_li, copy);
    // equality of lists of lists, via the descriptor
    dif_list *p = INTS(1), *q = INTS(1), *outer1 = NULL, *outer2 = NULL;
    dif_list_push(&ty_li, &outer1, &p);
    dif_list_push(&ty_li, &outer2, &q);
    CHECK(dif_list_eq(&ty_li, outer1, outer2));
    CHECK(dif_list_hash_t(&ty_lli, &outer1) == dif_list_hash_t(&ty_lli, &outer2));
    dif_list_dec(&ty_li, outer1);
    dif_list_dec(&ty_li, outer2);
}

static void test_arrays(void) {
    int64_t a[4] = {3, 1, 2, 1};
    int64_t one = 1;
    CHECK(dif_arr_index_of(&dif_ty_int, a, 4, &one) == 1);
    CHECK(dif_arr_count(&dif_ty_int, a, 4, &one) == 2);
    dif_arr_sort(&dif_ty_int, a, 4);
    CHECK(a[0] == 1 && a[1] == 1 && a[2] == 2 && a[3] == 3);
    dif_arr_reverse(&dif_ty_int, a, 4);
    CHECK(a[0] == 3 && a[3] == 1);
    dif_buf b = dif_buf_new();
    dif_arr_put(&dif_ty_int, &b, a, 4);
    CHECK_BUF(b, "[3, 2, 1, 1]");
    int64_t grid[2][2] = {{1, 2}, {3, 4}};
    DIF_CONST dif_ty row = DIF_ARRAY_TY(&dif_ty_int, 2, 2 * sizeof(int64_t));
    DIF_CONST dif_ty g = DIF_ARRAY_TY(&row, 2, sizeof grid);
    g.put(&g, &b, grid);
    CHECK_BUF(b, "[[1, 2], [3, 4]]");
    int64_t grid2[2][2] = {{1, 2}, {3, 4}};
    CHECK(g.eq(&g, grid, grid2));
    grid2[1][0] = 7;
    CHECK(!g.eq(&g, grid, grid2) && g.hash(&g, grid) != g.hash(&g, grid2));
    dif_list *s = dif_arr_slice(&dif_ty_int, a, 4, 1, 3, 1, 1);
    EXPECT_INTS(s, 2, 1);
    dif_list_dec(&dif_ty_int, s);
    // an array of texts: inc/dec through the descriptor
    dif_str texts[2] = {dif_cstr("x"), dif_cstr("y")};
    DIF_CONST dif_ty at = DIF_ARRAY_TY(&dif_ty_str, 2, sizeof texts);
    dif_array_inc_t(&at, texts);
    CHECK(*texts[0].rc == 2);
    dif_array_dec_t(&at, texts);
    dif_array_dec_t(&at, texts);
    EXPECT_PANIC("slice 3..5 out of bounds for length 4 (main.dif:1:1)", dif_arr_slice(&dif_ty_int, a, 4, 3, 5, 1, 1));
}

static void test_descriptors(void) {
    dif_buf b = dif_buf_new();
    int8_t i8 = -5;
    uint64_t u = UINT64_MAX;
    bool t = true;
    dif_ty_i8.put(&dif_ty_i8, &b, &i8);
    dif_put_c(&b, " ");
    dif_ty_u64.put(&dif_ty_u64, &b, &u);
    dif_put_c(&b, " ");
    dif_ty_bool.put(&dif_ty_bool, &b, &t);
    CHECK_BUF(b, "-5 18446744073709551615 true");
    dif_str s = DIF_S("a\"b");
    dif_ty_str.put(&dif_ty_str, &b, &s);
    CHECK_BUF(b, "\"a\\\"b\"");
    int64_t x = 3, y = 3, z = 4;
    CHECK(dif_ty_int.eq(&dif_ty_int, &x, &y) && !dif_ty_int.eq(&dif_ty_int, &x, &z));
    CHECK(dif_ty_int.cmp(&dif_ty_int, &x, &z) < 0 && dif_ty_int.cmp(&dif_ty_int, &z, &x) > 0);
    CHECK(dif_ty_int.hash(&dif_ty_int, &x) == dif_ty_int.hash(&dif_ty_int, &y) && dif_ty_int.hash(&dif_ty_int, &x) != dif_ty_int.hash(&dif_ty_int, &z));
    dif_str s1 = DIF_S("ab"), s2 = DIF_S("ab"), s3 = DIF_S("b");
    CHECK(dif_ty_str.eq(&dif_ty_str, &s1, &s2) && dif_ty_str.cmp(&dif_ty_str, &s1, &s3) < 0);
    CHECK(dif_ty_str.hash(&dif_ty_str, &s1) == dif_ty_str.hash(&dif_ty_str, &s2));
    uint8_t u1 = 200, u2 = 100;
    CHECK(dif_ty_u8.cmp(&dif_ty_u8, &u1, &u2) > 0); // unsigned compare
}

static void test_split_lines_join(void) {
    dif_list *p = dif_str_split(DIF_S("a,b,,c"), DIF_S(","), 1, 1);
    EXPECT_TEXT(&dif_ty_str, p, "[\"a\", \"b\", \"\", \"c\"]");
    dif_str j = dif_list_join(p, DIF_S("-"));
    CHECK_STR(j, "a-b--c");
    dif_str_dec(j);
    dif_list_dec(&dif_ty_str, p);
    p = dif_str_split(DIF_EMPTY, DIF_S(","), 1, 1);
    EXPECT_TEXT(&dif_ty_str, p, "[\"\"]");
    dif_list_dec(&dif_ty_str, p);
    p = dif_str_split(DIF_S("a--b---c"), DIF_S("--"), 1, 1);
    EXPECT_TEXT(&dif_ty_str, p, "[\"a\", \"b\", \"-c\"]");
    dif_list_dec(&dif_ty_str, p);
    p = dif_str_split(DIF_S(",a,"), DIF_S(","), 1, 1);
    EXPECT_TEXT(&dif_ty_str, p, "[\"\", \"a\", \"\"]");
    dif_list_dec(&dif_ty_str, p);
    dif_str big = dif_str_new("one two", 7); // pieces share the block
    p = dif_str_split(big, DIF_S(" "), 1, 1);
    CHECK(big.rc[0] == 3);
    dif_str_dec(big);
    EXPECT_TEXT(&dif_ty_str, p, "[\"one\", \"two\"]");
    dif_list_dec(&dif_ty_str, p);
    EXPECT_PANIC("split needs a non-empty separator (main.dif:2:3)", dif_str_split(DIF_S("a"), DIF_EMPTY, 2, 3));

    p = dif_str_lines(DIF_S("a\nb\n"));
    EXPECT_TEXT(&dif_ty_str, p, "[\"a\", \"b\"]");
    dif_list_dec(&dif_ty_str, p);
    p = dif_str_lines(DIF_S("a\r\nb\r\n\r\nc"));
    EXPECT_TEXT(&dif_ty_str, p, "[\"a\", \"b\", \"\", \"c\"]");
    dif_list_dec(&dif_ty_str, p);
    p = dif_str_lines(DIF_EMPTY);
    CHECK(p == NULL);
    p = dif_str_lines(DIF_S("\n"));
    EXPECT_TEXT(&dif_ty_str, p, "[\"\"]");
    dif_list_dec(&dif_ty_str, p);
    p = dif_str_lines(DIF_S("a\rb\r"));
    EXPECT_TEXT(&dif_ty_str, p, "[\"a\\rb\\r\"]"); // a lone \r stays
    dif_list_dec(&dif_ty_str, p);
    p = dif_str_lines(DIF_S("x\n\n"));
    EXPECT_TEXT(&dif_ty_str, p, "[\"x\", \"\"]");
    dif_list_dec(&dif_ty_str, p);
    j = dif_list_join(NULL, DIF_S(","));
    CHECK(j.n == 0);
}

int main(void) {
    test_push_pop();
    test_cow();
    test_ops();
    test_panics();
    test_struct_elements();
    test_nested();
    test_arrays();
    test_descriptors();
    test_split_lines_join();
    T_DONE();
}
