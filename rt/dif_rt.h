// dif_rt.h - runtime library of the difficulty language (C11, libc and libm only).
//
// Usage: define `static const char *const dif_file = "main.dif";` before including this
// header, link with -lm. Every function is `static inline`, so unused ones cost nothing.
//
// Ownership rules (the backend follows them, see the architecture note):
//  * Arguments are BORROWED: the callee never frees or keeps them, unless the function
//    is documented as "takes" (push, insert, set, ... move their element/key/value in).
//  * Results are OWNED (+1 reference): the caller releases them with the matching *_dec.
//  * Copying a value (assignment, argument without mut, return of a variable) is
//    `*_inc`; leaving a scope or overwriting a variable is `*_dec`.
//  * Pointers returned by `*_at`, `*_get`, `*_first`, ... point INTO the value and are
//    borrowed; they stay valid until the next write to that value.
//  * Mutators take a pointer to the variable (`dif_list **`) and do the copy-on-write
//    step themselves: a shared block (rc > 1) is copied first.
//  * Position arguments `l, c` (line, column) are only used for aborts (R14).
#ifndef DIF_RT_H
#define DIF_RT_H

#include <errno.h>
#include <inttypes.h>
#include <math.h>
#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>


#ifndef DIF_EXIT
#define DIF_EXIT exit // tests replace it with _exit (no leak check in forked children)
#endif
#define DIF_FN static inline
#define DIF_COLD static __attribute__((noinline, cold, unused))

// ---------------------------------------------------------------------------------------
// 1. Aborts and allocation
// ---------------------------------------------------------------------------------------

DIF_COLD _Noreturn void dif_panic(const char *msg, int line, int col) {
    fflush(stdout);
    fprintf(stderr, "panic: %s (%s:%d:%d)\n", msg, dif_file, line, col);
    fflush(stderr);
    DIF_EXIT(2);
}
DIF_COLD _Noreturn void dif_panicf(int line, int col, const char *fmt, ...) {
    char m[512];
    va_list ap;
    va_start(ap, fmt);
    vsnprintf(m, sizeof m, fmt, ap);
    va_end(ap);
    dif_panic(m, line, col);
}
// No position is known inside the allocator: the message names the file only.
DIF_COLD _Noreturn void dif_oom(void) {
    fflush(stdout);
    fprintf(stderr, "panic: out of memory (%s)\n", dif_file);
    fflush(stderr);
    DIF_EXIT(2);
}
__attribute__((malloc, alloc_size(1))) DIF_FN void *dif_alloc(size_t n) {
    void *p = malloc(n ? n : 1);
    if (!p) dif_oom();
    return p;
}
DIF_FN void *dif_realloc(void *p, size_t n) {
    p = realloc(p, n ? n : 1);
    if (!p) dif_oom();
    return p;
}
// n * size + extra with overflow check (overflow = out of memory)
DIF_FN size_t dif_bytes(int64_t n, size_t size, size_t extra) {
    size_t r;
    if (n < 0 || __builtin_mul_overflow((size_t)n, size, &r) || __builtin_add_overflow(r, extra, &r)) dif_oom();
    return r;
}

// ---------------------------------------------------------------------------------------
// 2. Integer arithmetic (R14, R29). `int` is int64_t; i8 ... u64 have a suffix.
// ---------------------------------------------------------------------------------------

DIF_FN int64_t dif_add(int64_t a, int64_t b, int l, int c) {
    int64_t r;
    if (__builtin_add_overflow(a, b, &r)) dif_panic("integer overflow in +", l, c);
    return r;
}
DIF_FN int64_t dif_sub(int64_t a, int64_t b, int l, int c) {
    int64_t r;
    if (__builtin_sub_overflow(a, b, &r)) dif_panic("integer overflow in -", l, c);
    return r;
}
DIF_FN int64_t dif_mul(int64_t a, int64_t b, int l, int c) {
    int64_t r;
    if (__builtin_mul_overflow(a, b, &r)) dif_panic("integer overflow in *", l, c);
    return r;
}
DIF_FN int64_t dif_neg(int64_t a, int l, int c) {
    if (a == INT64_MIN) dif_panic("integer overflow in -", l, c);
    return -a;
}
DIF_FN int64_t dif_div(int64_t a, int64_t b, int l, int c) {
    if (b == 0) dif_panic("division by zero", l, c);
    if (a == INT64_MIN && b == -1) dif_panic("integer overflow in /", l, c);
    return a / b;
}
DIF_FN int64_t dif_mod(int64_t a, int64_t b, int l, int c) {
    if (b == 0) dif_panic("division by zero", l, c);
    return b == -1 ? 0 : a % b;
}
DIF_FN void dif_shift(int64_t b, int l, int c) {
    if (b < 0 || b > 63) dif_panicf(l, c, "shift by %" PRId64 " out of range 0..63", b);
}
DIF_FN int64_t dif_shl(int64_t a, int64_t b, int l, int c) {
    dif_shift(b, l, c);
    return (int64_t)((uint64_t)a << b);
}
DIF_FN int64_t dif_shr(int64_t a, int64_t b, int l, int c) {
    dif_shift(b, l, c);
    return a >> b;
}
DIF_FN int64_t dif_wadd(int64_t a, int64_t b) { return (int64_t)((uint64_t)a + (uint64_t)b); }
DIF_FN int64_t dif_wsub(int64_t a, int64_t b) { return (int64_t)((uint64_t)a - (uint64_t)b); }
DIF_FN int64_t dif_wmul(int64_t a, int64_t b) { return (int64_t)((uint64_t)a * (uint64_t)b); }

// The same for the other widths: dif_add_u8(a, b, l, c), dif_shl_i16(a, b, l, c), ...
#define DIF_ARITH(S, T, BITS, MIN)                                                                  \
    DIF_FN T dif_add_##S(T a, T b, int l, int c) {                                                 \
        T r;                                                                                        \
        if (__builtin_add_overflow(a, b, &r)) dif_panic("integer overflow in +", l, c);            \
        return r;                                                                                   \
    }                                                                                               \
    DIF_FN T dif_sub_##S(T a, T b, int l, int c) {                                                 \
        T r;                                                                                        \
        if (__builtin_sub_overflow(a, b, &r)) dif_panic("integer overflow in -", l, c);            \
        return r;                                                                                   \
    }                                                                                               \
    DIF_FN T dif_mul_##S(T a, T b, int l, int c) {                                                 \
        T r;                                                                                        \
        if (__builtin_mul_overflow(a, b, &r)) dif_panic("integer overflow in *", l, c);            \
        return r;                                                                                   \
    }                                                                                               \
    DIF_FN T dif_neg_##S(T a, int l, int c) {                                                      \
        T r;                                                                                        \
        if (__builtin_sub_overflow((T)0, a, &r)) dif_panic("integer overflow in -", l, c);         \
        return r;                                                                                   \
    }                                                                                               \
    DIF_FN T dif_div_##S(T a, T b, int l, int c) {                                                 \
        if (b == 0) dif_panic("division by zero", l, c);                                           \
        if ((T)-1 < 0 && a == MIN && b == (T)-1) dif_panic("integer overflow in /", l, c);         \
        return (T)(a / b);                                                                          \
    }                                                                                               \
    DIF_FN T dif_mod_##S(T a, T b, int l, int c) {                                                 \
        if (b == 0) dif_panic("division by zero", l, c);                                           \
        return ((T)-1 < 0 && b == (T)-1) ? (T)0 : (T)(a % b);                                       \
    }                                                                                               \
    DIF_FN void dif_shift_##S(T b, int l, int c) {                                                 \
        if (((T)-1 < 0 && (b & ((T)1 << (BITS - 1))) != 0) || b > (T)(BITS - 1)) {                 \
            if ((T)-1 < 0) dif_panicf(l, c, "shift by %lld out of range 0..%d", (long long)b, BITS - 1); \
            dif_panicf(l, c, "shift by %llu out of range 0..%d", (unsigned long long)b, BITS - 1); \
        }                                                                                           \
    }                                                                                               \
    DIF_FN T dif_shl_##S(T a, T b, int l, int c) {                                                 \
        dif_shift_##S(b, l, c);                                                                     \
        return (T)((uint64_t)a << b);                                                               \
    }                                                                                               \
    DIF_FN T dif_shr_##S(T a, T b, int l, int c) {                                                 \
        dif_shift_##S(b, l, c);                                                                     \
        return (T)(a >> b);                                                                         \
    }                                                                                               \
    DIF_FN T dif_wadd_##S(T a, T b) { return (T)((uint64_t)a + (uint64_t)b); }                      \
    DIF_FN T dif_wsub_##S(T a, T b) { return (T)((uint64_t)a - (uint64_t)b); }                      \
    DIF_FN T dif_wmul_##S(T a, T b) { return (T)((uint64_t)a * (uint64_t)b); }
DIF_ARITH(i8, int8_t, 8, INT8_MIN)
DIF_ARITH(i16, int16_t, 16, INT16_MIN)
DIF_ARITH(i32, int32_t, 32, INT32_MIN)
DIF_ARITH(u8, uint8_t, 8, 0)
DIF_ARITH(u16, uint16_t, 16, 0)
DIF_ARITH(u32, uint32_t, 32, 0)
DIF_ARITH(u64, uint64_t, 64, 0)

// abs() of the signed types; the smallest value aborts (R14)
DIF_FN int64_t dif_abs(int64_t a, int64_t min, int l, int c) {
    if (a == min) dif_panic("integer overflow in abs", l, c);
    return a < 0 ? -a : a;
}

// ---------------------------------------------------------------------------------------
// 3. str: {p, n, rc}. p points into a heap block {rc, cap, bytes...} (rc != NULL) or to
//    static data (rc == NULL, e.g. literals). Substrings share the block. A text is
//    immutable; "copy-on-write" appears as `dif_str_append`: an unshared text with room
//    left grows in place, otherwise the append builds a new block.
// ---------------------------------------------------------------------------------------

typedef struct { const char *p; int64_t n; int64_t *rc; } dif_str;
#define DIF_S(lit) ((dif_str){(lit), (int64_t)sizeof(lit) - 1, NULL})
#define DIF_EMPTY ((dif_str){"", 0, NULL})

#define DIF_DATA(k) ((char *)(k) + 16)
#define DIF_BLK(p) ((int64_t *)((char *)(p) - 16))
DIF_FN int64_t *dif_blk_new(int64_t cap) {
    int64_t *b = dif_alloc(dif_bytes(cap, 1, 16));
    b[0] = 1;
    b[1] = cap;
    return b;
}
DIF_FN dif_str dif_str_inc(dif_str s) {
    if (s.rc) ++*s.rc;
    return s;
}
DIF_FN void dif_str_dec(dif_str s) {
    if (s.rc && --*s.rc == 0) free(s.rc);
}
// new text with a copy of the bytes
DIF_FN dif_str dif_str_new(const char *p, int64_t n) {
    if (n <= 0) return DIF_EMPTY;
    int64_t *b = dif_blk_new(n);
    memcpy(DIF_DATA(b), p, (size_t)n);
    return (dif_str){DIF_DATA(b), n, b};
}
// bytes a..b-1 sharing the block (caller checked the bounds)
DIF_FN dif_str dif_str_sub(dif_str s, int64_t a, int64_t b) {
    if (a >= b) return DIF_EMPTY;
    if (s.rc) ++*s.rc;
    return (dif_str){s.p + a, b - a, s.rc};
}
DIF_FN dif_str dif_cstr(const char *z) { return dif_str_new(z, (int64_t)strlen(z)); }
DIF_FN bool dif_eq(dif_str a, dif_str b) {
    return a.n == b.n && (a.n == 0 || memcmp(a.p, b.p, (size_t)a.n) == 0);
}
DIF_FN int dif_cmp(dif_str a, dif_str b) {
    int64_t n = a.n < b.n ? a.n : b.n;
    int r = n ? memcmp(a.p, b.p, (size_t)n) : 0;
    return r ? r : (a.n > b.n) - (a.n < b.n);
}
// `s += t`: in place if nothing else shares the block and it has room
DIF_FN void dif_str_append(dif_str *s, dif_str t) {
    if (t.n == 0) return;
    if (s->rc && *s->rc == 1 && s->p == DIF_DATA(s->rc) && s->n + t.n <= s->rc[1]) {
        memcpy((char *)s->p + s->n, t.p, (size_t)t.n);
        s->n += t.n;
        return;
    }
    int64_t cap = s->n + t.n;
    if (cap > INT64_MAX / 2) dif_oom();
    int64_t *b = dif_blk_new(cap * 2);
    memcpy(DIF_DATA(b), s->p, (size_t)s->n);
    memcpy(DIF_DATA(b) + s->n, t.p, (size_t)t.n);
    dif_str r = {DIF_DATA(b), cap, b};
    dif_str_dec(*s);
    *s = r;
}
DIF_FN dif_str dif_cat(dif_str a, dif_str b) {
    if (a.n == 0) return dif_str_inc(b);
    if (b.n == 0) return dif_str_inc(a);
    int64_t n;
    if (__builtin_add_overflow(a.n, b.n, &n)) dif_oom();
    int64_t *k = dif_blk_new(n);
    char *d = DIF_DATA(k);
    memcpy(d, a.p, (size_t)a.n);
    memcpy(d + a.n, b.p, (size_t)b.n);
    return (dif_str){d, n, k};
}

// Text builder (interpolation, text forms). `dif_take` turns it into a str without a copy.
typedef struct { char *p; int64_t n, cap; } dif_buf;
DIF_FN dif_buf dif_buf_new(void) { return (dif_buf){0, 0, 0}; }
DIF_FN void dif_put(dif_buf *b, const char *p, int64_t n) {
    if (n <= 0) return;
    if (b->n + n > b->cap) {
        int64_t cap = (b->n + n) * 2 + 16;
        if (!b->p) {
            b->p = DIF_DATA(dif_blk_new(cap));
        } else {
            int64_t *k = dif_realloc(DIF_BLK(b->p), dif_bytes(cap, 1, 16));
            k[1] = cap;
            b->p = DIF_DATA(k);
        }
        b->cap = cap;
    }
    memcpy(b->p + b->n, p, (size_t)n);
    b->n += n;
}
DIF_FN void dif_buf_free(dif_buf *b) {
    if (b->p) free(DIF_BLK(b->p));
    *b = dif_buf_new();
}
DIF_FN dif_str dif_take(dif_buf *b) {
    if (!b->p || b->n == 0) {
        dif_buf_free(b);
        return DIF_EMPTY;
    }
    int64_t *k = DIF_BLK(b->p);
    k[1] = b->cap;
    dif_str s = {b->p, b->n, k};
    *b = dif_buf_new();
    return s;
}
DIF_FN void dif_put_str(dif_buf *b, dif_str s) { dif_put(b, s.p, s.n); }
DIF_FN void dif_put_c(dif_buf *b, const char *z) { dif_put(b, z, (int64_t)strlen(z)); }
DIF_FN void dif_put_i64(dif_buf *b, int64_t v) {
    char t[24];
    int n = snprintf(t, sizeof t, "%" PRId64, v);
    dif_put(b, t, n);
}
DIF_FN void dif_put_u64(dif_buf *b, uint64_t v) {
    char t[24];
    int n = snprintf(t, sizeof t, "%" PRIu64, v);
    dif_put(b, t, n);
}
#define dif_put_int dif_put_i64
DIF_FN void dif_put_bool(dif_buf *b, bool v) { dif_put(b, v ? "true" : "false", v ? 4 : 5); }

// Escaped body of a text form: `"` or `'` (q), `\`, \n \t \r \0 and other controls < 0x20
// as \u{..}; braces stay as they are (R28).
DIF_FN void dif_put_esc(dif_buf *b, const char *p, int64_t n, char q) {
    for (int64_t i = 0; i < n; i++) {
        unsigned char ch = (unsigned char)p[i];
        if (ch == (unsigned char)q || ch == '\\') {
            char e[2] = {'\\', (char)ch};
            dif_put(b, e, 2);
        } else if (ch == '\n') dif_put(b, "\\n", 2);
        else if (ch == '\t') dif_put(b, "\\t", 2);
        else if (ch == '\r') dif_put(b, "\\r", 2);
        else if (ch == 0) dif_put(b, "\\0", 2);
        else if (ch < 0x20) {
            char e[16];
            int k = snprintf(e, sizeof e, "\\u{%x}", ch);
            dif_put(b, e, k);
        } else dif_put(b, p + i, 1);
    }
}
DIF_FN void dif_put_quoted(dif_buf *b, dif_str s) {
    dif_put(b, "\"", 1);
    dif_put_esc(b, s.p, s.n, '"');
    dif_put(b, "\"", 1);
}

// Output. stdout is flushed before stderr is written and before stdin is read.
DIF_FN void dif_print_str(dif_str s) {
    if (s.n) fwrite(s.p, 1, (size_t)s.n, stdout);
    putchar('\n');
}
DIF_FN void dif_eprint_str(dif_str s) {
    fflush(stdout);
    if (s.n) fwrite(s.p, 1, (size_t)s.n, stderr);
    fputc('\n', stderr);
    fflush(stderr);
}
DIF_FN void dif_print_int(int64_t v) { printf("%" PRId64 "\n", v); }
DIF_FN void dif_print_bool(bool v) { puts(v ? "true" : "false"); }

// Search and slicing (bytes). `find` gives the byte position or -1.
DIF_FN int64_t dif_find(dif_str s, dif_str t) {
    if (t.n == 0) return 0;
    for (int64_t i = 0; i + t.n <= s.n; i++) {
        const char *q = memchr(s.p + i, t.p[0], (size_t)(s.n - i - t.n + 1));
        if (!q) return -1;
        i = q - s.p;
        if (memcmp(q, t.p, (size_t)t.n) == 0) return i;
    }
    return -1;
}
DIF_FN bool dif_contains(dif_str s, dif_str t) { return dif_find(s, t) >= 0; }
DIF_FN bool dif_starts_with(dif_str s, dif_str t) { return t.n <= s.n && memcmp(s.p, t.p, (size_t)t.n) == 0; }
DIF_FN bool dif_ends_with(dif_str s, dif_str t) {
    return t.n <= s.n && memcmp(s.p + s.n - t.n, t.p, (size_t)t.n) == 0;
}
DIF_FN bool dif_is_cont(char ch) { return ((unsigned char)ch & 0xC0) == 0x80; }
DIF_FN dif_str dif_str_slice(dif_str s, int64_t a, int64_t b, int l, int c) {
    if (a < 0 || a > b || b > s.n)
        dif_panicf(l, c, "slice %" PRId64 "..%" PRId64 " out of bounds for length %" PRId64, a, b, s.n);
    if ((a < s.n && dif_is_cont(s.p[a])) || (b < s.n && dif_is_cont(s.p[b])))
        dif_panicf(l, c, "slice %" PRId64 "..%" PRId64 " is not on character boundaries", a, b);
    return dif_str_sub(s, a, b);
}
DIF_FN dif_str dif_str_repeat(dif_str s, int64_t n, int l, int c) {
    if (n < 0) dif_panicf(l, c, "repeat count %" PRId64 " is negative", n);
    if (n == 0 || s.n == 0) return DIF_EMPTY;
    int64_t total;
    if (__builtin_mul_overflow(s.n, n, &total)) dif_oom();
    int64_t *k = dif_blk_new(total);
    for (int64_t i = 0; i < n; i++) memcpy(DIF_DATA(k) + i * s.n, s.p, (size_t)s.n);
    return (dif_str){DIF_DATA(k), total, k};
}
DIF_FN dif_str dif_str_replace(dif_str s, dif_str from, dif_str to, int l, int c) {
    if (from.n == 0) dif_panic("replace needs a non-empty text to find", l, c);
    dif_buf b = dif_buf_new();
    int64_t i = 0;
    for (;;) {
        dif_str rest = {s.p + i, s.n - i, NULL};
        int64_t k = dif_find(rest, from);
        if (k < 0) break;
        dif_put(&b, rest.p, k);
        dif_put_str(&b, to);
        i += k + from.n;
    }
    if (i == 0) {
        dif_buf_free(&b);
        return dif_str_inc(s);
    }
    dif_put(&b, s.p + i, s.n - i);
    return dif_take(&b);
}

#endif // DIF_RT_H
