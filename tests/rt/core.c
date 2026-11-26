// Allocation, aborts, integer arithmetic, str, builder.
#include "t.h"

static void test_alloc(void) {
    void *p = dif_alloc(0);
    CHECK(p != NULL);
    p = dif_realloc(p, 100);
    CHECK(p != NULL);
    free(p);
    EXPECT_PANIC("out of memory (main.dif)", void *volatile q = dif_alloc((size_t)-1 / 2); (void)q);
    EXPECT_PANIC("out of memory (main.dif)", dif_bytes(INT64_MAX, 16, 0));
    EXPECT_PANIC("out of memory (main.dif)", dif_bytes(-1, 1, 0));
    CHECK(dif_bytes(10, 8, 24) == 104);
    EXPECT_PANIC("boom 7 (main.dif:3:9)", dif_panicf(3, 9, "boom %d", 7));
    // stdout is written before the message (R14, 4.1)
    EXPECT_EXIT(2, "before\npanic: x (main.dif:1:2)\n", printf("before\n"); dif_panic("x", 1, 2));
}

static void test_int64(void) {
    CHECK(dif_add(1, 2, 0, 0) == 3);
    CHECK(dif_sub(INT64_MIN, -1, 0, 0) == INT64_MIN + 1);
    CHECK(dif_mul(-4, 5, 0, 0) == -20);
    CHECK(dif_div(-7, 2, 0, 0) == -3);
    CHECK(dif_mod(-7, 3, 0, 0) == -1);
    CHECK(dif_mod(INT64_MIN, -1, 0, 0) == 0);
    CHECK(dif_shl(1, 62, 0, 0) == 4611686018427387904);
    CHECK(dif_shl(1, 63, 0, 0) == INT64_MIN);
    CHECK(dif_shr(-8, 1, 0, 0) == -4);
    CHECK(dif_wadd(INT64_MAX, 1) == INT64_MIN);
    CHECK(dif_wsub(INT64_MIN, 1) == INT64_MAX);
    CHECK(dif_wmul(INT64_MAX, 2) == -2);
    CHECK(dif_abs(-5, INT64_MIN, 0, 0) == 5);
    EXPECT_PANIC("integer overflow in + (main.dif:3:15)", dif_add(INT64_MAX, 1, 3, 15));
    EXPECT_PANIC("integer overflow in - (main.dif:1:1)", dif_sub(INT64_MIN, 1, 1, 1));
    EXPECT_PANIC("integer overflow in * (main.dif:1:1)", dif_mul(INT64_MAX, 2, 1, 1));
    EXPECT_PANIC("integer overflow in - (main.dif:1:1)", dif_neg(INT64_MIN, 1, 1));
    EXPECT_PANIC("division by zero (main.dif:1:1)", dif_div(1, 0, 1, 1));
    EXPECT_PANIC("division by zero (main.dif:1:1)", dif_mod(1, 0, 1, 1));
    EXPECT_PANIC("integer overflow in / (main.dif:1:1)", dif_div(INT64_MIN, -1, 1, 1));
    EXPECT_PANIC("shift by 64 out of range 0..63 (main.dif:3:13)", dif_shl(1, 64, 3, 13));
    EXPECT_PANIC("shift by -1 out of range 0..63 (main.dif:1:1)", dif_shr(1, -1, 1, 1));
    EXPECT_PANIC("integer overflow in abs (main.dif:1:1)", dif_abs(INT64_MIN, INT64_MIN, 1, 1));
}

static void test_widths(void) {
    CHECK(dif_add_u8(200, 55, 0, 0) == 255);
    CHECK(dif_wadd_u8(200, 100) == 44);
    CHECK(dif_wsub_u8(0, 1) == 255);
    CHECK(dif_wmul_i8(100, 3) == 44);
    CHECK(dif_sub_u32(5, 5, 0, 0) == 0);
    CHECK(dif_mul_i16(-200, 100, 0, 0) == -20000);
    CHECK(dif_div_i8(-128, 2, 0, 0) == -64);
    CHECK(dif_mod_i8(-128, -1, 0, 0) == 0);
    CHECK(dif_mod_u16(7, 4, 0, 0) == 3);
    CHECK(dif_shl_u8(255, 7, 0, 0) == 128);
    CHECK(dif_shl_i8(1, 7, 0, 0) == -128);
    CHECK(dif_shr_i8(-128, 7, 0, 0) == -1);
    CHECK(dif_shr_u8(128, 7, 0, 0) == 1);
    CHECK(dif_shl_u64(1, 63, 0, 0) == 9223372036854775808u);
    CHECK(dif_add_u64(UINT64_MAX - 1, 1, 0, 0) == UINT64_MAX);
    CHECK(dif_neg_i32(5, 0, 0) == -5);
    EXPECT_PANIC("integer overflow in + (main.dif:4:13)", dif_add_u8(200, 100, 4, 13));
    EXPECT_PANIC("integer overflow in - (main.dif:1:1)", dif_sub_u8(0, 1, 1, 1));
    EXPECT_PANIC("integer overflow in * (main.dif:1:1)", dif_mul_u32(65536, 65536, 1, 1));
    EXPECT_PANIC("integer overflow in / (main.dif:1:1)", dif_div_i8(-128, -1, 1, 1));
    EXPECT_PANIC("integer overflow in - (main.dif:1:1)", dif_neg_i8(-128, 1, 1));
    EXPECT_PANIC("division by zero (main.dif:1:1)", dif_div_u16(1, 0, 1, 1));
    EXPECT_PANIC("shift by 8 out of range 0..7 (main.dif:1:1)", dif_shl_u8(1, 8, 1, 1));
    EXPECT_PANIC("shift by 32 out of range 0..31 (main.dif:1:1)", dif_shr_i32(1, 32, 1, 1));
    EXPECT_PANIC("shift by -1 out of range 0..15 (main.dif:1:1)", dif_shl_i16(1, -1, 1, 1));
}

static void test_str_rc(void) {
    dif_str a = dif_str_new("hello", 5);
    CHECK(*a.rc == 1);
    dif_str b = dif_str_inc(a);
    CHECK(*a.rc == 2 && b.p == a.p);
    dif_str_dec(b);
    CHECK(*a.rc == 1);
    // literals have no counter; inc/dec are no-ops
    dif_str lit = DIF_S("lit");
    dif_str_inc(lit);
    dif_str_dec(lit);
    CHECK(lit.rc == NULL && lit.n == 3);
    dif_str e = dif_str_new("x", 0);
    CHECK(e.n == 0 && e.rc == NULL);
    dif_str_dec(e);
    dif_str_dec(a);
}

static void test_str_append(void) {
    dif_str s = DIF_S("ab");
    dif_str_append(&s, DIF_S(""));
    CHECK(s.rc == NULL);
    dif_str_append(&s, DIF_S("cd")); // literal: new block, with room
    CHECK(s.rc != NULL && *s.rc == 1);
    CHECK_STR(s, "abcd");
    const char *p0 = s.p;
    dif_str_append(&s, DIF_S("e")); // unshared, room left: in place
    CHECK(s.p == p0);
    CHECK_STR(s, "abcde");
    dif_str copy = dif_str_inc(s); // shared: the append must not change the copy
    dif_str_append(&s, DIF_S("F"));
    CHECK(s.p != p0);
    CHECK_STR(copy, "abcde");
    CHECK_STR(s, "abcdeF");
    dif_str_dec(copy);
    dif_str_append(&s, s); // s += s
    CHECK_STR(s, "abcdeFabcdeF");
    for (int i = 0; i < 1000; i++) dif_str_append(&s, DIF_S("xy"));
    CHECK(s.n == 12 + 2000);
    dif_str sub = dif_str_sub(s, 0, 3); // a slice shares the block, so an append copies
    dif_str_append(&sub, DIF_S("!"));
    CHECK_STR(sub, "abc!");
    CHECK(s.p[3] == 'd');
    dif_str_dec(sub);
    dif_str_dec(s);
    dif_str t = dif_str_new("abcdef", 6);
    dif_str u = dif_str_sub(t, 0, 3);
    dif_str_dec(t); // u is now the only owner of the block and still a prefix
    dif_str_append(&u, DIF_S("XYZ"));
    CHECK_STR(u, "abcXYZ");
    dif_str_dec(u);
}

static void test_str_ops(void) {
    dif_str c = dif_cat(DIF_S("foo"), DIF_S("bar"));
    CHECK_STR(c, "foobar");
    dif_str d = dif_cat(c, DIF_EMPTY);
    CHECK(d.p == c.p && *c.rc == 2);
    dif_str_dec(d);
    dif_str_dec(c);
    CHECK(dif_eq(DIF_S("a"), DIF_S("a")) && !dif_eq(DIF_S("a"), DIF_S("b")) && !dif_eq(DIF_S("a"), DIF_S("ab")));
    CHECK(dif_eq(DIF_EMPTY, DIF_S("")));
    CHECK(dif_cmp(DIF_S("a"), DIF_S("b")) < 0 && dif_cmp(DIF_S("b"), DIF_S("a")) > 0);
    CHECK(dif_cmp(DIF_S("ab"), DIF_S("a")) > 0 && dif_cmp(DIF_S("a"), DIF_S("ab")) < 0);
    CHECK(dif_cmp(DIF_S("a"), DIF_S("a")) == 0 && dif_cmp(DIF_EMPTY, DIF_EMPTY) == 0);
    CHECK(dif_cmp(DIF_S("\xc3\xa4"), DIF_S("z")) > 0); // bytewise: ä after z

    CHECK(dif_find(DIF_S("Hallo"), DIF_S("ll")) == 2);
    CHECK(dif_find(DIF_S("x"), DIF_S("y")) == -1);
    CHECK(dif_find(DIF_S("abc"), DIF_S("")) == 0);
    CHECK(dif_find(DIF_EMPTY, DIF_S("")) == 0);
    CHECK(dif_find(DIF_EMPTY, DIF_S("a")) == -1);
    CHECK(dif_find(DIF_S("aab"), DIF_S("ab")) == 1);
    CHECK(dif_find(DIF_S("ab"), DIF_S("abc")) == -1);
    CHECK(dif_find(DIF_S("abab"), DIF_S("b")) == 1);
    CHECK(dif_contains(DIF_S("abc"), DIF_S("c")) && dif_contains(DIF_S("abc"), DIF_S("")));
    CHECK(dif_starts_with(DIF_S("abc"), DIF_S("ab")) && !dif_starts_with(DIF_S("abc"), DIF_S("bc")));
    CHECK(dif_starts_with(DIF_S("abc"), DIF_S("")) && !dif_starts_with(DIF_S("a"), DIF_S("ab")));
    CHECK(dif_ends_with(DIF_S("abc"), DIF_S("bc")) && !dif_ends_with(DIF_S("abc"), DIF_S("ab")));
    CHECK(dif_ends_with(DIF_S("abc"), DIF_S("")) && !dif_ends_with(DIF_S("c"), DIF_S("bc")));

    dif_str r = dif_str_repeat(DIF_S("ab"), 3, 0, 0);
    CHECK_STR(r, "ababab");
    dif_str_dec(r);
    r = dif_str_repeat(DIF_S("ab"), 0, 0, 0);
    CHECK(r.n == 0);
    r = dif_str_repeat(DIF_EMPTY, 5, 0, 0);
    CHECK(r.n == 0);
    EXPECT_PANIC("repeat count -1 is negative (main.dif:2:7)", dif_str_repeat(DIF_S("a"), -1, 2, 7));
    EXPECT_PANIC("out of memory (main.dif)", dif_str_repeat(DIF_S("abcd"), INT64_MAX / 2, 1, 1));

    r = dif_str_replace(DIF_S("a-b-c"), DIF_S("-"), DIF_S("+"), 0, 0);
    CHECK_STR(r, "a+b+c");
    dif_str_dec(r);
    r = dif_str_replace(DIF_S("aaa"), DIF_S("aa"), DIF_S("b"), 0, 0); // no overlap
    CHECK_STR(r, "ba");
    dif_str_dec(r);
    r = dif_str_replace(DIF_S("abc"), DIF_S("x"), DIF_S("y"), 0, 0);
    CHECK_STR(r, "abc");
    dif_str_dec(r);
    r = dif_str_replace(DIF_S("abc"), DIF_S("abc"), DIF_S(""), 0, 0);
    CHECK(r.n == 0);
    r = dif_str_replace(DIF_EMPTY, DIF_S("a"), DIF_S("b"), 0, 0);
    CHECK(r.n == 0);
    EXPECT_PANIC("replace needs a non-empty text to find (main.dif:5:9)",
                 dif_str_replace(DIF_S("a"), DIF_EMPTY, DIF_S("b"), 5, 9));
}

static void test_str_slice(void) {
    dif_str s = DIF_S("H\xc3\xa4llo"); // "Hällo", ä = bytes 1..2
    dif_str r = dif_str_slice(s, 0, 1, 0, 0);
    CHECK_STR(r, "H");
    r = dif_str_slice(s, 1, 3, 0, 0);
    CHECK_STR(r, "\xc3\xa4");
    r = dif_str_slice(s, 0, 6, 0, 0);
    CHECK(r.n == 6);
    r = dif_str_slice(s, 6, 6, 0, 0);
    CHECK(r.n == 0);
    r = dif_str_slice(s, 0, 0, 0, 0);
    CHECK(r.n == 0);
    dif_str h = dif_str_new("Hallo", 5);
    r = dif_str_slice(h, 1, 3, 0, 0);
    CHECK_STR(r, "al");
    CHECK(*h.rc == 2); // shares the block
    dif_str_dec(h);
    CHECK_STR(r, "al"); // the slice keeps the block alive
    dif_str_dec(r);
    EXPECT_PANIC("slice 1..2 is not on character boundaries (main.dif:3:13)", dif_str_slice(s, 1, 2, 3, 13));
    EXPECT_PANIC("slice 2..3 is not on character boundaries (main.dif:1:1)", dif_str_slice(s, 2, 3, 1, 1));
    EXPECT_PANIC("slice -1..2 out of bounds for length 6 (main.dif:1:1)", dif_str_slice(s, -1, 2, 1, 1));
    EXPECT_PANIC("slice 3..2 out of bounds for length 6 (main.dif:1:1)", dif_str_slice(s, 3, 2, 1, 1));
    EXPECT_PANIC("slice 0..7 out of bounds for length 6 (main.dif:1:1)", dif_str_slice(s, 0, 7, 1, 1));
}

static void test_buf(void) {
    dif_buf b = dif_buf_new();
    dif_str e = dif_take(&b);
    CHECK(e.n == 0);
    dif_put(&b, "", 0);
    e = dif_take(&b);
    CHECK(e.n == 0 && e.rc == NULL);
    dif_put_str(&b, DIF_S("n="));
    dif_put_i64(&b, -42);
    dif_put_c(&b, " ");
    dif_put_u64(&b, UINT64_MAX);
    dif_put_c(&b, " ");
    dif_put_bool(&b, true);
    dif_put_bool(&b, false);
    CHECK_BUF(b, "n=-42 18446744073709551615 truefalse");
    for (int i = 0; i < 10000; i++) dif_put(&b, "0123456789", 10);
    dif_str big = dif_take(&b);
    CHECK(big.n == 100000 && *big.rc == 1 && big.rc[1] >= big.n);
    dif_str_dec(big);
    dif_put_i64(&b, INT64_MIN);
    CHECK_BUF(b, "-9223372036854775808");
    dif_put_c(&b, "abandoned");
    dif_buf_free(&b);
    CHECK(b.p == NULL);
}

static void test_quoted(void) {
    dif_buf b = dif_buf_new();
    dif_put_quoted(&b, DIF_S("a\"b\\c\nd\te\rf{g}"));
    CHECK_BUF(b, "\"a\\\"b\\\\c\\nd\\te\\rf{g}\"");
    dif_str z = dif_str_new("x\0y\x01\x1f", 5);
    dif_put_quoted(&b, z);
    dif_str_dec(z);
    CHECK_BUF(b, "\"x\\0y\\u{1}\\u{1f}\"");
    dif_put_quoted(&b, DIF_EMPTY);
    CHECK_BUF(b, "\"\"");
    dif_put_quoted(&b, DIF_S("'\xc3\xa4"));
    CHECK_BUF(b, "\"'\xc3\xa4\"");
    dif_put_esc(&b, "it's \"x\"", 8, '\'');
    CHECK_BUF(b, "it\\'s \"x\"");
}

static void test_print(void) {
    EXPECT_EXIT(0, "hi\n7\ntrue\nfalse\n\n", dif_print_str(DIF_S("hi")); dif_print_int(7); dif_print_bool(true);
                dif_print_bool(false); dif_print_str(DIF_EMPTY); fflush(stdout));
    EXPECT_EXIT(0, "out\nerr\n", printf("out\n"); dif_eprint_str(DIF_S("err")));
}

int main(void) {
    test_alloc();
    test_int64();
    test_widths();
    test_str_rc();
    test_str_append();
    test_str_ops();
    test_str_slice();
    test_buf();
    test_quoted();
    test_print();
    T_DONE();
}
