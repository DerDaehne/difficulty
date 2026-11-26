// UTF-8 checks, runes, Unicode classes, case mapping, trim.
#include "t.h"

static bool valid(const char *s, int n) { return dif_utf8_valid(s, n); }
#define V(lit) valid(lit, (int)sizeof(lit) - 1)

static void test_valid(void) {
    CHECK(V("") && V("abc") && V("\xc3\xa4") && V("\xe2\x82\xac") && V("\xf0\x9f\x98\x80"));
    CHECK(V("\x00") && V("\x7f") && V("\xc2\x80") && V("\xdf\xbf") && V("\xe0\xa0\x80") && V("\xef\xbf\xbf"));
    CHECK(V("\xf0\x90\x80\x80") && V("\xf4\x8f\xbf\xbf"));
    CHECK(!V("\x80") && !V("\xbf") && !V("a\x80"));                   // stray continuation bytes
    CHECK(!V("\xc0\x80") && !V("\xc1\xbf") && !V("\xc0\xaf"));        // overlong 2 bytes
    CHECK(!V("\xe0\x80\x80") && !V("\xe0\x9f\xbf"));                  // overlong 3 bytes
    CHECK(!V("\xf0\x80\x80\x80") && !V("\xf0\x8f\xbf\xbf"));          // overlong 4 bytes
    CHECK(!V("\xed\xa0\x80") && !V("\xed\xbf\xbf"));                  // surrogates
    CHECK(V("\xed\x9f\xbf") && V("\xee\x80\x80"));                    // just outside
    CHECK(!V("\xf4\x90\x80\x80") && !V("\xf5\x80\x80\x80") && !V("\xff") && !V("\xfe")); // beyond 10FFFF
    CHECK(!V("\xc3") && !V("\xe2\x82") && !V("\xf0\x9f\x98") && !V("a\xe2"));               // truncated
    CHECK(!V("\xc3\x28") && !V("\xe2\x28\xa1") && !V("\xf0\x28\x8c\xbc"));                  // bad continuation
    CHECK(!V("\xc3\xc3"));
}

static void test_codecs(void) {
    char b[4];
    uint32_t cp;
    int bad = 0;
    for (uint32_t c = 0; c <= 0x10FFFF; c++) {
        if (!dif_rune_ok(c)) {
            continue;
        }
        int k = dif_utf8_encode(c, b);
        uint32_t d = 0;
        if (dif_utf8_decode(b, k, &d) != k || d != c || !dif_utf8_valid(b, k)) bad++;
        if (k > 1 && dif_utf8_decode(b, k - 1, &d) != 0) bad++; // truncated
    }
    CHECK(bad == 0);
    CHECK(dif_utf8_decode("\xc3\xa4x", 3, &cp) == 2 && cp == 0xE4);
    CHECK(dif_rune_ok(0) && dif_rune_ok(0x10FFFF) && dif_rune_ok(0xD7FF) && dif_rune_ok(0xE000));
    CHECK(!dif_rune_ok(0xD800) && !dif_rune_ok(0xDFFF) && !dif_rune_ok(0x110000) && !dif_rune_ok(-1));
    dif_str s = dif_rune_str(0x1F600);
    CHECK_STR(s, "\xf0\x9f\x98\x80");
    dif_str_dec(s);
    dif_buf buf = dif_buf_new();
    dif_put_rune(&buf, 'a');
    dif_put_rune(&buf, 0xE4);
    CHECK_BUF(buf, "a\xc3\xa4");
}

static void test_runes(void) {
    dif_list *l = dif_str_runes(DIF_S("a\xc3\xa4\xe2\x82\xac\xf0\x9f\x98\x80"));
    CHECK(dif_list_len(l) == 4);
    const uint32_t *d = dif_list_data(l);
    CHECK(d[0] == 'a' && d[1] == 0xE4 && d[2] == 0x20AC && d[3] == 0x1F600);
    dif_list_dec(&dif_ty_rune, l);
    CHECK(dif_str_runes(DIF_EMPTY) == NULL);
    // text forms: allone raw, inside a list quoted
    l = dif_str_runes(DIF_S("a'\n\\\"\xc3\xa4"));
    dif_buf b = dif_buf_new();
    dif_list_put(&dif_ty_rune, &b, l);
    CHECK_BUF(b, "['a', '\\'', '\\n', '\\\\', '\"', '\xc3\xa4']");
    dif_list_dec(&dif_ty_rune, l);
    uint32_t x = 1;
    dif_ty_rune.put(&dif_ty_rune, &b, &x);
    CHECK_BUF(b, "'\\u{1}'");
    uint32_t r1 = 'a', r2 = 'b';
    CHECK(dif_ty_rune.cmp(&dif_ty_rune, &r1, &r2) < 0 && !dif_ty_rune.eq(&dif_ty_rune, &r1, &r2));
    CHECK(dif_ty_rune.hash(&dif_ty_rune, &r1) != dif_ty_rune.hash(&dif_ty_rune, &r2));
}

static void test_classes(void) {
    CHECK(dif_rune_is_letter('a') && dif_rune_is_letter('Z') && dif_rune_is_letter(0xE4) && dif_rune_is_letter(0xDF));
    CHECK(dif_rune_is_letter(0x4E2D) && dif_rune_is_letter(0x3042) && dif_rune_is_letter(0x03A9) && dif_rune_is_letter(0x01C5));
    CHECK(dif_rune_is_letter(0xAA) && dif_rune_is_letter(0x2B0)); // Lo, Lm
    CHECK(!dif_rune_is_letter('1') && !dif_rune_is_letter(' ') && !dif_rune_is_letter(0xBD) && !dif_rune_is_letter(0x1F600));
    CHECK(!dif_rune_is_letter('_') && !dif_rune_is_letter(0x20AC) && !dif_rune_is_letter(0x0301)); // Mn is not L
    CHECK(dif_rune_is_letter(0x20000) && dif_rune_is_letter(0x10FFFD) == false);
    CHECK(dif_rune_is_digit('0') && dif_rune_is_digit('9') && dif_rune_is_digit(0x0663) && dif_rune_is_digit(0xFF15));
    CHECK(!dif_rune_is_digit(0xBD) && !dif_rune_is_digit('a') && !dif_rune_is_digit(0xB2) && !dif_rune_is_digit('/'));
    CHECK(!dif_rune_is_digit(':') && !dif_rune_is_digit(0x2160)); // roman numeral is Nl
    CHECK(dif_rune_is_space(' ') && dif_rune_is_space('\t') && dif_rune_is_space('\n') && dif_rune_is_space('\r'));
    CHECK(dif_rune_is_space(0x0B) && dif_rune_is_space(0x0C) && dif_rune_is_space(0x85) && dif_rune_is_space(0xA0));
    CHECK(dif_rune_is_space(0x1680) && dif_rune_is_space(0x2000) && dif_rune_is_space(0x200A) && dif_rune_is_space(0x2028));
    CHECK(dif_rune_is_space(0x2029) && dif_rune_is_space(0x202F) && dif_rune_is_space(0x205F) && dif_rune_is_space(0x3000));
    CHECK(!dif_rune_is_space(0x200B) && !dif_rune_is_space(0x180E) && !dif_rune_is_space(0) && !dif_rune_is_space('a'));
    CHECK(!dif_rune_is_space(0x1F) && !dif_rune_is_space(0x21) && !dif_rune_is_space(0x2007 + 100));
    CHECK(!dif_rune_is_letter(0x10FFFF) && !dif_rune_is_digit(0x10FFFF) && !dif_rune_is_space(0x10FFFF));
    CHECK(!dif_rune_is_letter(0) && !dif_rune_is_digit(0));
}

static void case_is(const char *in, const char *low, const char *up) {
    dif_str s = {in, (int64_t)strlen(in), NULL};
    dif_str l = dif_str_lower(s), u = dif_str_upper(s);
    CHECK_STR(l, low);
    CHECK_STR(u, up);
    dif_str_dec(l);
    dif_str_dec(u);
}

static void test_case(void) {
    case_is("Hello", "hello", "HELLO");
    case_is("", "", "");
    case_is("\xc3\x84pfel", "\xc3\xa4pfel", "\xc3\x84PFEL");                    // Äpfel
    case_is("stra\xc3\x9f" "e", "stra\xc3\x9f" "e", "STRA\xc3\x9f" "E");        // straße: ß stays (no multi-char)
    case_is("\xce\xa3\xce\xaf\xcf\x83\xcf\x85\xcf\x86\xcf\x82", "\xcf\x83\xce\xaf\xcf\x83\xcf\x85\xcf\x86\xcf\x82",
            "\xce\xa3\xce\x8a\xce\xa3\xce\xa5\xce\xa6\xce\xa3");               // Σίσυφς: final sigma upper = Σ
    case_is("\xc7\x85", "\xc7\x86", "\xc7\x84");                               // titlecase Dž
    case_is("\xc4\xb0", "i", "\xc4\xb0");                                       // İ -> i (simple mapping)
    case_is("\xc4\xb1", "\xc4\xb1", "I");                                       // dotless i upper = I
    case_is("\xe1\xbe\x80", "\xe1\xbe\x80", "\xe1\xbe\x88");                    // U+1F80 upper U+1F88
    case_is("\xd0\x94\xd0\xb4", "\xd0\xb4\xd0\xb4", "\xd0\x94\xd0\x94");        // Cyrillic
    case_is("\xf0\x90\x90\x80", "\xf0\x90\x90\xa8", "\xf0\x90\x90\x80");        // Deseret (outside the BMP)
    case_is("123 !?\xe2\x82\xac\xf0\x9f\x98\x80", "123 !?\xe2\x82\xac\xf0\x9f\x98\x80", "123 !?\xe2\x82\xac\xf0\x9f\x98\x80");
    // a text without change is returned as it is (same block)
    dif_str s = dif_str_new("abc", 3), l = dif_str_lower(s);
    CHECK(l.p == s.p && *s.rc == 2);
    dif_str_dec(l);
    dif_str_dec(s);
    CHECK(dif_rune_lower('A') == 'a' && dif_rune_lower('a') == 'a' && dif_rune_upper('z') == 'Z');
    CHECK(dif_rune_upper(0xB5) == 0x39C && dif_rune_upper(0xFF) == 0x178 && dif_rune_lower(0x178) == 0xFF);
    CHECK(dif_rune_upper(0x10FFFF) == 0x10FFFF && dif_rune_lower(0) == 0);
    // every pair maps back (not required by Unicode, but true for these letters)
    CHECK(dif_rune_lower(dif_rune_upper(0x3B1)) == 0x3B1 && dif_rune_upper(dif_rune_lower(0x410)) == 0x410);
    // the whole range must not crash
    uint32_t sum = 0;
    for (uint32_t c = 0; c <= 0x10FFFF; c++) sum += dif_rune_lower(c) + dif_rune_upper(c) + dif_rune_is_letter(c);
    CHECK(sum != 0);
}

static void trim_is(const char *in, const char *both, const char *start, const char *end) {
    dif_str s = {in, (int64_t)strlen(in), NULL};
    dif_str a = dif_str_trim(s, true, true), b = dif_str_trim(s, true, false), c = dif_str_trim(s, false, true);
    CHECK_STR(a, both);
    CHECK_STR(b, start);
    CHECK_STR(c, end);
    dif_str_dec(a);
    dif_str_dec(b);
    dif_str_dec(c);
}

static void test_trim(void) {
    trim_is("  Hallo, Welt  ", "Hallo, Welt", "Hallo, Welt  ", "  Hallo, Welt");
    trim_is("", "", "", "");
    trim_is("   ", "", "", "");
    trim_is("\t\n x \r\n", "x", "x \r\n", "\t\n x");
    trim_is("x", "x", "x", "x");
    trim_is("\xc2\xa0x\xe3\x80\x80", "x", "x\xe3\x80\x80", "\xc2\xa0x");        // NBSP, ideographic space
    trim_is("\xe2\x80\x8b" "x", "\xe2\x80\x8b" "x", "\xe2\x80\x8b" "x", "\xe2\x80\x8b" "x"); // ZWSP is not space
    trim_is(" \xc3\xa4 ", "\xc3\xa4", "\xc3\xa4 ", " \xc3\xa4");
    trim_is("a b", "a b", "a b", "a b");
    trim_is("\xe2\x80\xa8\xe2\x80\xa9x\xe2\x80\x82", "x", "x\xe2\x80\x82", "\xe2\x80\xa8\xe2\x80\xa9x");
    dif_str s = dif_str_new("  ab ", 5), t = dif_str_trim(s, true, true); // shares, no copy
    CHECK(t.p == s.p + 2 && *s.rc == 2);
    dif_str_dec(t);
    dif_str_dec(s);
}

int main(void) {
    test_valid();
    test_codecs();
    test_runes();
    test_classes();
    test_case();
    test_trim();
    T_DONE();
}
