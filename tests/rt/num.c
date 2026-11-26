// f64 text form and formats, conversions, parse_int, parse_f64.
#include "t.h"
#include <float.h>

static void f64_is(double x, const char *want, int line) {
    dif_buf b = dif_buf_new();
    dif_put_f64(&b, x);
    dif_str s = dif_take(&b);
    t_count++;
    if (s.n != (int64_t)strlen(want) || memcmp(s.p, want, (size_t)s.n) != 0) {
        fprintf(stderr, "num.c:%d: %.17g printed as \"%.*s\", expected \"%s\"\n", line, x, (int)s.n, s.p, want);
        t_fail++;
    }
    dif_str_dec(s);
}
#define F64(x, want) f64_is(x, want, __LINE__)

static void test_f64_text(void) {
    F64(0.0, "0");
    F64(-0.0, "-0");
    F64(1.0 / 0.0, "inf");
    F64(-1.0 / 0.0, "-inf");
    F64(NAN, "NaN");
    F64(-NAN, "NaN");
    F64(1, "1");
    F64(-1, "-1");
    F64(100, "100");
    F64(22.5, "22.5");
    F64(0.5, "0.5");
    F64(3.5, "3.5");
    F64(0.1 + 0.2, "0.30000000000000004");
    F64(0.1, "0.1");
    F64(1.0 / 3, "0.3333333333333333");
    F64(2.0 / 3, "0.6666666666666666");
    F64(12345.678, "12345.678");
    F64(123456789.0, "123456789");
    F64(9007199254740992.0, "9007199254740992");
    F64(9007199254740993.0, "9007199254740992"); // the literal rounds to the neighbour
    F64(1e15, "1000000000000000");
    F64(1e20, "100000000000000000000");
    F64(1.5e20, "150000000000000000000");
    F64(123456789012345678.0, "123456789012345680");
    F64(1e21, "1e21");
    F64(1.5e21, "1.5e21");
    F64(-1e21, "-1e21");
    F64(1.2345678901234567e25, "1.2345678901234566e25");
    F64(1e100, "1e100");
    F64(1e-6, "0.000001");
    F64(1.5e-6, "0.0000015");
    F64(1.25e-5, "0.0000125");
    F64(-1e-6, "-0.000001");
    F64(9.99e-7, "9.99e-7");
    F64(1e-7, "1e-7");
    F64(1.5e-7, "1.5e-7");
    F64(1.2345e-10, "1.2345e-10");
    F64(DBL_MAX, "1.7976931348623157e308");
    F64(DBL_MIN, "2.2250738585072014e-308");
    F64(4.9406564584124654e-324, "5e-324");
    F64(-DBL_MAX, "-1.7976931348623157e308");
    F64(0.000123, "0.000123");
    F64(100000000000000000000.0 * 10, "1e21");
    F64(999999999999999900000.0, "999999999999999900000");
    F64(1e21 - 1e5, "999999999999999900000"); // the nearest double below 1e21
    F64(5e-7, "5e-7");
    F64(123.456, "123.456");
    F64(2.5, "2.5");
    F64(1.7976931348623157e308 / 2, "8.988465674311579e307");
}

// shortest: one digit fewer must not read back as the same value
static int digits_of(const char *s) {
    int n = 0, started = 0, last = 0;
    for (; *s && *s != 'e'; s++)
        if (*s >= '0' && *s <= '9') {
            if (*s != '0') started = 1;
            if (started) n++;
            if (*s != '0') last = n;
        }
    return last;
}

static void test_f64_roundtrip(void) {
    uint64_t st = 0x9E3779B97F4A7C15ULL;
    int bad = 0, notshort = 0;
    for (int i = 0; i < 400000; i++) {
        st ^= st << 13; st ^= st >> 7; st ^= st << 17; // xorshift64, fixed seed
        uint64_t u = st;
        double x;
        if (i % 4 == 1) u = (u & 0x800FFFFFFFFFFFFFULL) | ((uint64_t)(1023 - 60 + (int)(st >> 58) * 2) << 52); // modest exponents
        if (i % 4 == 2) { x = (double)(int64_t)(st >> 20) / 1000.0; memcpy(&u, &x, 8); }                       // "human" decimals
        if (i % 4 == 3) { x = (double)(int32_t)(st >> 40) * 1e-9; memcpy(&u, &x, 8); }
        memcpy(&x, &u, 8);
        if (x != x) continue;
        dif_buf b = dif_buf_new();
        dif_put_f64(&b, x);
        dif_str s = dif_take(&b);
        double back;
        bool ok = dif_parse_f64(s, &back);
        if (!isinf(x) && x != 0) {
            if (!ok || memcmp(&back, &x, 8) != 0) {
                if (bad++ < 5) fprintf(stderr, "no roundtrip: %.17g -> %.*s\n", x, (int)s.n, s.p);
            }
            char z[64], t[64];
            snprintf(z, sizeof z, "%.*s", (int)s.n, s.p);
            int nd = digits_of(z);
            if (nd > 1) {
                snprintf(t, sizeof t, "%.*e", nd - 2, x);
                if (strtod(t, NULL) == x && notshort++ < 5) fprintf(stderr, "not shortest: %.17g -> %s\n", x, z);
            }
        }
        dif_str_dec(s);
    }
    CHECK(bad == 0);
    CHECK(notshort == 0);
    // every power of ten and of two, both signs and neighbours
    for (int e = -320; e <= 308; e++) {
        double x = strtod((char[]){'1', 'e', (char)0}, NULL);
        char t[16];
        snprintf(t, sizeof t, "1e%d", e);
        x = strtod(t, NULL);
        double xs[3] = {x, nextafter(x, 0), nextafter(x, INFINITY)};
        for (int k = 0; k < 3; k++) {
            dif_buf b = dif_buf_new();
            dif_put_f64(&b, -xs[k]);
            dif_str s = dif_take(&b);
            double back = 0;
            CHECK(dif_parse_f64(s, &back) && back == -xs[k]);
            dif_str_dec(s);
        }
    }
    // the border of the notation: -6 <= exponent <= 20 positional
    for (int e = -8; e <= 22; e++) {
        char t[16];
        snprintf(t, sizeof t, "1e%d", e);
        dif_buf b = dif_buf_new();
        dif_put_f64(&b, strtod(t, NULL));
        dif_str s = dif_take(&b);
        CHECK((memchr(s.p, 'e', (size_t)s.n) != NULL) == (e < -6 || e > 20));
        dif_str_dec(s);
    }
}

static void fmt_is(double x, int prec, const char *want, int line) {
    dif_buf b = dif_buf_new();
    dif_put_f64_fmt(&b, x, prec);
    dif_str s = dif_take(&b);
    t_count++;
    if (s.n != (int64_t)strlen(want) || memcmp(s.p, want, (size_t)s.n) != 0) {
        fprintf(stderr, "num.c:%d: format %d of %.17g is \"%.*s\", expected \"%s\"\n", line, prec, x, (int)s.n, s.p, want);
        t_fail++;
    }
    dif_str_dec(s);
}
#define FMT(x, prec, want) fmt_is(x, prec, want, __LINE__)

static void test_formats(void) {
    FMT(2.5, 0, "2");        // exact middle: to even
    FMT(3.5, 0, "4");
    FMT(0.5, 0, "0");
    FMT(1.005, 2, "1.00");   // the double is below 1.005
    FMT(22.5, 1, "22.5");
    FMT(9.0 / 40.0 * 100, 1, "22.5");
    FMT(1.0, 3, "1.000");
    FMT(-0.0, 1, "-0.0");
    FMT(1e21, 2, "1000000000000000000000.00");
    FMT(0.1, 20, "0.10000000000000000555");
    FMT(DBL_MAX, 20, "179769313486231570814527423731704356798070567525844996598917476803157260780028538760589558632766878171540458953514382464234321326889464182768467546703537516986049910576551282076245490090389328944075868508455133942304583236903222948165808559332123348274797826204144723168738177180919299881250404026184124858368.00000000000000000000");
    FMT(1.0 / 0.0, 2, "inf");
    FMT(-1.0 / 0.0, 2, "-inf");
    FMT(NAN, 2, "NaN");
    FMT(NAN, -1, "NaN");
    FMT(22.5, -1, "22.5");
    FMT(100000, -1, "100000");
    FMT(1000000, -1, "1e+06");
    FMT(1e21, -1, "1e+21");
    FMT(0.0001, -1, "0.0001");
    FMT(0.00001, -1, "1e-05");
    FMT(1.0 / 3, -1, "0.333333");
    FMT(1234567.0, -1, "1.23457e+06");
    FMT(-1.0 / 0.0, -1, "-inf");
    FMT(0.0, -1, "0");
}

static void test_f64_type(void) {
    double a = 1.0, b = 2.0, nan = NAN, z = 0.0, nz = -0.0;
    CHECK(dif_ty_f64.eq(&dif_ty_f64, &z, &nz) && !dif_ty_f64.eq(&dif_ty_f64, &nan, &nan));
    CHECK(dif_ty_f64.cmp(&dif_ty_f64, &a, &b) < 0 && dif_ty_f64.cmp(&dif_ty_f64, &nan, &b) > 0);
    CHECK(dif_ty_f64.cmp(&dif_ty_f64, &b, &nan) < 0 && dif_ty_f64.cmp(&dif_ty_f64, &nan, &nan) == 0);
    dif_nan_seen = 0;
    CHECK(dif_ty_f64.hash(&dif_ty_f64, &z) == dif_ty_f64.hash(&dif_ty_f64, &nz) && !dif_nan_seen);
    dif_ty_f64.hash(&dif_ty_f64, &nan);
    CHECK(dif_nan_seen == 1);
    // sort: NaN behind all numbers, stable for the rest
    double v[6] = {3, NAN, -1, 1.0 / 0.0, -1.0 / 0.0, 0};
    dif_arr_sort(&dif_ty_f64, v, 6);
    CHECK(v[0] == -1.0 / 0.0 && v[1] == -1 && v[2] == 0 && v[3] == 3 && v[4] == 1.0 / 0.0 && v[5] != v[5]);
    // a list of f64 in text form, equal lists with NaN are never equal
    dif_list *l = NULL;
    for (int i = 0; i < 3; i++) dif_list_push(&dif_ty_f64, &l, &(double){i * 0.5});
    dif_buf buf = dif_buf_new();
    dif_list_put(&dif_ty_f64, &buf, l);
    CHECK_BUF(buf, "[0, 0.5, 1]");
    CHECK(dif_list_eq(&dif_ty_f64, l, l));
    dif_list_push(&dif_ty_f64, &l, &nan);
    CHECK(!dif_list_eq(&dif_ty_f64, l, l));
    dif_list_dec(&dif_ty_f64, l);
    // the methods of 4.3 are the C functions
    CHECK(round(2.5) == 3 && round(-2.5) == -3 && trunc(-2.7) == -2 && floor(-2.5) == -3 && ceil(2.1) == 3);
    CHECK(isnan(sqrt(-1.0)) && fmin(NAN, 1) == 1 && fmax(1, NAN) == 1 && fmod(-7.0, 3.0) == -1 && fabs(-0.0) == 0);
    CHECK(isinf(round(1.0 / 0.0)) && isnan(fmod(1.0, 0.0)));
}

static void test_conversions(void) {
    // fits
    CHECK(dif_fits_s(255, 8, false) && !dif_fits_s(256, 8, false) && !dif_fits_s(-1, 8, false));
    CHECK(dif_fits_s(127, 8, true) && !dif_fits_s(128, 8, true) && dif_fits_s(-128, 8, true) && !dif_fits_s(-129, 8, true));
    CHECK(dif_fits_s(INT64_MAX, 64, true) && dif_fits_s(INT64_MAX, 64, false) && !dif_fits_s(-1, 64, false));
    CHECK(dif_fits_s(65535, 16, false) && !dif_fits_s(65536, 16, false) && dif_fits_s(4294967295LL, 32, false));
    CHECK(!dif_fits_s(4294967296LL, 32, false) && !dif_fits_s(2147483648LL, 32, true) && dif_fits_s(-2147483648LL, 32, true));
    CHECK(dif_fits_u(255, 8, false) && !dif_fits_u(256, 8, false) && dif_fits_u(127, 8, true) && !dif_fits_u(128, 8, true));
    CHECK(dif_fits_u(UINT64_MAX, 64, false) && !dif_fits_u(UINT64_MAX, 64, true) && dif_fits_u(INT64_MAX, 64, true));
    CHECK(!dif_fits_u((uint64_t)INT64_MAX + 1, 64, true) && dif_fits_u(32767, 16, true) && !dif_fits_u(32768, 16, true));
    CHECK(dif_conv_s(-128, 8, true, 1, 1) == -128 && dif_conv_u(300, 16, false, 1, 1) == 300);
    EXPECT_PANIC("300 does not fit in u8 (main.dif:5:11)", dif_conv_s(300, 8, false, 5, 11));
    EXPECT_PANIC("-1 does not fit in u64 (main.dif:1:1)", dif_conv_s(-1, 64, false, 1, 1));
    EXPECT_PANIC("128 does not fit in i8 (main.dif:1:1)", dif_conv_s(128, 8, true, 1, 1));
    EXPECT_PANIC("128512 does not fit in u8 (main.dif:1:1)", dif_conv_u(128512, 8, false, 1, 1));
    EXPECT_PANIC("18446744073709551615 does not fit in int (main.dif:1:1)", dif_conv_u(UINT64_MAX, 64, true, 1, 1));
    // f64 -> integer
    CHECK(dif_f2s(23.0, 64, true, 1, 1) == 23 && dif_f2s(-0.0, 8, false, 1, 1) == 0 && dif_f2s(-128.0, 8, true, 1, 1) == -128);
    CHECK(dif_f2s(-9223372036854775808.0, 64, true, 1, 1) == INT64_MIN && dif_f2u(18446744073709549568.0, 64, false, 1, 1) == 18446744073709549568ULL);
    CHECK(dif_f2u(255.0, 8, false, 1, 1) == 255 && dif_f2s(1e15, 64, true, 1, 1) == 1000000000000000);
    EXPECT_PANIC("2.5 is not an exact int (main.dif:3:11)", dif_f2s(2.5, 64, true, 3, 11));
    EXPECT_PANIC("NaN is not an exact int (main.dif:1:1)", dif_f2s(NAN, 64, true, 1, 1));
    EXPECT_PANIC("inf is not an exact int (main.dif:1:1)", dif_f2s(1.0 / 0.0, 64, true, 1, 1));
    EXPECT_PANIC("-inf is not an exact u8 (main.dif:1:1)", dif_f2u(-1.0 / 0.0, 8, false, 1, 1));
    EXPECT_PANIC("1e30 does not fit in int (main.dif:1:1)", dif_f2s(1e30, 64, true, 1, 1));
    EXPECT_PANIC("9223372036854776000 does not fit in int (main.dif:1:1)", dif_f2s(9223372036854775808.0, 64, true, 1, 1));
    EXPECT_PANIC("-1 does not fit in u32 (main.dif:1:1)", dif_f2u(-1.0, 32, false, 1, 1));
    EXPECT_PANIC("256 does not fit in u8 (main.dif:1:1)", dif_f2u(256.0, 8, false, 1, 1));
    EXPECT_PANIC("0.5 is not an exact u8 (main.dif:1:1)", dif_f2u(0.5, 8, false, 1, 1));
    // integer -> f64
    CHECK(dif_s2f(300, 1, 1) == 300.0 && dif_s2f(9007199254740992LL, 1, 1) == 9007199254740992.0 && dif_s2f(-9007199254740992LL, 1, 1) == -9007199254740992.0);
    CHECK(dif_s2f(INT64_MIN, 1, 1) == -9223372036854775808.0 && dif_u2f(1ULL << 63, 1, 1) == 9223372036854775808.0);
    CHECK(dif_u2f(0, 1, 1) == 0.0);
    EXPECT_PANIC("9007199254740993 is not an exact f64 (main.dif:2:3)", dif_s2f(9007199254740993LL, 2, 3));
    EXPECT_PANIC("9223372036854775807 is not an exact f64 (main.dif:1:1)", dif_s2f(INT64_MAX, 1, 1));
    EXPECT_PANIC("18446744073709551615 is not an exact f64 (main.dif:1:1)", dif_u2f(UINT64_MAX, 1, 1));
    // integer -> rune
    CHECK(dif_rune_s(0x1F600, 1, 1) == 0x1F600 && dif_rune_u(0, 1, 1) == 0 && dif_rune_u(0x10FFFF, 1, 1) == 0x10FFFF);
    EXPECT_PANIC("55296 is not a valid rune (main.dif:4:4)", dif_rune_s(55296, 4, 4));
    EXPECT_PANIC("-1 is not a valid rune (main.dif:1:1)", dif_rune_s(-1, 1, 1));
    EXPECT_PANIC("1114112 is not a valid rune (main.dif:1:1)", dif_rune_u(0x110000, 1, 1));
    EXPECT_PANIC("18446744073709551615 is not a valid rune (main.dif:1:1)", dif_rune_u(UINT64_MAX, 1, 1));
}

static bool pi(const char *s, int64_t want) {
    int64_t v = 12345;
    bool ok = dif_parse_int((dif_str){s, (int64_t)strlen(s), NULL}, &v);
    return ok && v == want;
}
static bool pi_none(const char *s) {
    int64_t v = 12345;
    return !dif_parse_int((dif_str){s, (int64_t)strlen(s), NULL}, &v) && v == 12345;
}

static void test_parse_int(void) {
    CHECK(pi("42", 42) && pi("0", 0) && pi("-0", 0) && pi("+7", 7) && pi("-1", -1) && pi("007", 7) && pi("+007", 7));
    CHECK(pi("9223372036854775807", INT64_MAX) && pi("-9223372036854775808", INT64_MIN));
    CHECK(pi("00000000000000000000000000000000001", 1) && pi("-0000000000000000000000000000000", 0));
    CHECK(pi_none("9223372036854775808") && pi_none("-9223372036854775809") && pi_none("99999999999999999999"));
    CHECK(pi_none("18446744073709551616") && pi_none("18446744073709551615"));
    CHECK(pi_none("") && pi_none("+") && pi_none("-") && pi_none("--1") && pi_none("+-1") && pi_none("1-"));
    CHECK(pi_none(" 1") && pi_none("1 ") && pi_none("1_000") && pi_none("1.0") && pi_none("1e3") && pi_none("0x10"));
    CHECK(pi_none("4x") && pi_none("x4") && pi_none("\xd9\xa3") && pi_none("1\n") && pi_none("--") && pi_none("\xef\xbc\x91"));
    CHECK(dif_parse_int(DIF_S("42"), &(int64_t){0}));
    CHECK(!dif_parse_int(DIF_EMPTY, &(int64_t){0}));
}

static bool pf(const char *s, double want) {
    double v = 777;
    bool ok = dif_parse_f64((dif_str){s, (int64_t)strlen(s), NULL}, &v);
    return ok && (v == want || (v != v && want != want)) && signbit(v) == signbit(want);
}
static bool pf_none(const char *s) {
    double v = 777;
    return !dif_parse_f64((dif_str){s, (int64_t)strlen(s), NULL}, &v) && v == 777;
}

static void test_parse_f64(void) {
    CHECK(pf("1", 1) && pf("-1.5e2", -150) && pf("+2.5", 2.5) && pf("0.5", 0.5) && pf("007.50", 7.5) && pf("1E3", 1000));
    CHECK(pf("1e+3", 1000) && pf("1e-3", 0.001) && pf("1.5E-3", 0.0015) && pf("0", 0) && pf("-0", -0.0) && pf("0.0", 0.0));
    CHECK(pf("0e99999", 0) && pf("0.000e-99999", 0) && pf("-0.0", -0.0) && pf("00012", 12));
    CHECK(pf("0.30000000000000004", 0.1 + 0.2) && pf("1.7976931348623157e308", DBL_MAX) && pf("2.2250738585072014e-308", DBL_MIN));
    CHECK(pf("5e-324", 4.9406564584124654e-324) && pf("1e-320", 1e-320) && pf("123456789012345678901234567890", 1.2345678901234568e29));
    CHECK(pf("9007199254740993", 9007199254740992.0) && pf("0.1", 0.1) && pf("1e22", 1e22) && pf("2.5e-3", 0.0025));
    CHECK(pf("inf", INFINITY) && pf("+inf", INFINITY) && pf("-inf", -INFINITY) && pf("NaN", NAN));
    CHECK(pf("1.00000000000000011102230246251565404236316680908203125", 1.0));                // exact middle: to even
    CHECK(pf("1.00000000000000011102230246251565404236316680908203124", 1.0));                // just below: to 1
    CHECK(pf("1.00000000000000011102230246251565404236316680908203125000000000001", 1.0000000000000002));
    CHECK(pf_none("1e400") && pf_none("-1e400") && pf_none("1e-400") && pf_none("0.0000001e-400") && pf_none("1e99999999999999999999"));
    CHECK(pf_none("1.8e308") && pf_none("-1.8e308") && pf_none("2e-324") && pf("3e-324", 4.9406564584124654e-324)); // below / above half of 5e-324
    CHECK(pf_none("1e-400") && pf_none("9e-400"));
    CHECK(pf_none("") && pf_none(".5") && pf_none("5.") && pf_none("+") && pf_none("-") && pf_none("e5") && pf_none("1e") && pf_none("1e+"));
    CHECK(pf_none(" 1") && pf_none("1 ") && pf_none("1_0") && pf_none("1,5") && pf_none("0x10") && pf_none("0x1p3") && pf_none("1.5.2"));
    CHECK(pf_none("Inf") && pf_none("INF") && pf_none("infinity") && pf_none("nan") && pf_none("+NaN") && pf_none("-NaN") && pf_none("NAN"));
    CHECK(pf_none("1e5.5") && pf_none("--1") && pf_none("1f") && pf_none("1d") && pf_none("1\n") && pf_none("\xd9\xa3") && pf_none("1e1_0"));
    CHECK(pf_none("in") && pf_none("+-inf") && pf_none("i") && pf_none("N") && pf_none("-"));
}

int main(void) {
    test_f64_text();
    test_f64_roundtrip();
    test_formats();
    test_f64_type();
    test_conversions();
    test_parse_int();
    test_parse_f64();
    T_DONE();
}
