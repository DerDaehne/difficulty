// io: files, one shared stdin stream; os: args, env, exit.
#include "t.h"
#include <sys/stat.h>

static char tmpdir[512];
static dif_str path_of(const char *name) {
    char p[800];
    snprintf(p, sizeof p, "%s/%s-%d", tmpdir, name, (int)getpid());
    return dif_cstr(p);
}
static void put_file(dif_str path, const char *bytes, size_t n) {
    char *z = dif_cpath(path);
    FILE *f = fopen(z, "wb");
    fwrite(bytes, 1, n, f);
    fclose(f);
    free(z);
}
static void rm(dif_str path) {
    char *z = dif_cpath(path);
    chmod(z, 0600);
    remove(z);
    free(z);
}
static void feed(const char *bytes, size_t n) { // makes `bytes` the content of stdin
    dif_str p = path_of("stdin");
    put_file(p, bytes, n);
    char *z = dif_cpath(p);
    CHECK(freopen(z, "rb", stdin) != NULL);
    free(z);
    rm(p);
    dif_str_dec(p);
}
#define FEED(lit) feed(lit, sizeof(lit) - 1)

static void test_files(void) {
    dif_str p = path_of("a.txt"), text;
    dif_ioerr e;
    put_file(p, "h\xc3\xa4llo\n\0x", 9);
    CHECK(dif_io_read_file(p, &text, &e) && text.n == 9 && memcmp(text.p, "h\xc3\xa4llo\n\0x", 9) == 0);
    dif_str_dec(text);
    put_file(p, "", 0);
    CHECK(dif_io_read_file(p, &text, &e) && text.n == 0);
    // a large file (several read chunks)
    char *big = dif_alloc(100000);
    memset(big, 'x', 100000);
    put_file(p, big, 100000);
    CHECK(dif_io_read_file(p, &text, &e) && text.n == 100000 && text.p[99999] == 'x');
    dif_str_dec(text);
    free(big);
    // invalid UTF-8
    put_file(p, "ok\xff", 3);
    CHECK(!dif_io_read_file(p, &text, &e) && e.kind == DIF_IO_INVALID_UTF8 && dif_eq(e.path, p) && e.reason.n == 0);
    dif_ioerr_dec(&e);
    // write: create, overwrite with something shorter, bytes of the text
    CHECK(dif_io_write_file(p, DIF_S("longer text\n"), &e));
    CHECK(dif_io_write_file(p, DIF_S("\xc3\xa4"), &e));
    CHECK(dif_io_read_file(p, &text, &e));
    CHECK_STR(text, "\xc3\xa4");
    dif_str_dec(text);
    CHECK(dif_io_write_file(p, DIF_EMPTY, &e) && dif_io_read_file(p, &text, &e) && text.n == 0);
    // permissions
    char *z = dif_cpath(p);
    chmod(z, 0);
    if (geteuid() != 0) {
        CHECK(!dif_io_read_file(p, &text, &e) && e.kind == DIF_IO_NOPERM && dif_eq(e.path, p));
        dif_ioerr_dec(&e);
        CHECK(!dif_io_write_file(p, DIF_S("x"), &e) && e.kind == DIF_IO_NOPERM);
        dif_ioerr_dec(&e);
    }
    free(z);
    rm(p);
    // missing file, missing directory
    CHECK(!dif_io_read_file(p, &text, &e) && e.kind == DIF_IO_NOTFOUND && dif_eq(e.path, p) && e.reason.n == 0);
    dif_ioerr_dec(&e);
    dif_str dir = path_of("no-such-dir"), q = dif_cat(dir, DIF_S("/f"));
    dif_str_dec(dir);
    CHECK(!dif_io_write_file(q, DIF_S("x"), &e) && e.kind == DIF_IO_NOTFOUND && dif_eq(e.path, q));
    dif_ioerr_dec(&e);
    dif_str_dec(q);
    // a directory: other, with the reason of the system
    dif_str d = dif_cstr(tmpdir);
    CHECK(!dif_io_read_file(d, &text, &e) && e.kind == DIF_IO_OTHER && e.reason.n > 0 && dif_eq(e.path, d));
    dif_ioerr_dec(&e);
    CHECK(!dif_io_write_file(d, DIF_S("x"), &e) && e.kind == DIF_IO_OTHER && e.reason.n > 0);
    dif_ioerr_dec(&e);
    dif_str_dec(d);
    // NUL in a path is no path
    dif_str nul = dif_str_new("a\0b", 3);
    CHECK(!dif_io_read_file(nul, &text, &e) && e.kind == DIF_IO_OTHER);
    dif_ioerr_dec(&e);
    dif_str_dec(nul);
    CHECK(!dif_io_read_file(DIF_EMPTY, &text, &e) && e.kind == DIF_IO_NOTFOUND);
    dif_ioerr_dec(&e);
    dif_str_dec(p);
}

static void line_is(const char *want) {
    dif_str s;
    dif_ioerr e;
    int r = dif_io_read_line(&s, &e);
    CHECK(r == 1);
    if (r == 1) {
        CHECK_STR(s, want);
        dif_str_dec(s);
    }
}
static void eof(void) {
    dif_str s;
    dif_ioerr e;
    CHECK(dif_io_read_line(&s, &e) == 0);
}

static void test_lines(void) {
    FEED("a\r\nb\n\nlast");
    line_is("a");
    line_is("b");
    line_is("");
    line_is("last"); // no newline at the end
    eof();
    eof();
    FEED("");
    eof();
    FEED("\n");
    line_is("");
    eof();
    FEED("x\r");
    line_is("x\r"); // a lone \r at the end stays
    FEED("a\rb\r\r\n\xc3\xa4\n");
    line_is("a\rb\r");
    line_is("\xc3\xa4");
    eof();
    // invalid UTF-8: error, the line is used up, the next one reads fine
    FEED("ok\n\xff\xfe\nnext\n\xc3");
    line_is("ok");
    dif_str s;
    dif_ioerr e;
    CHECK(dif_io_read_line(&s, &e) == -1 && e.kind == DIF_IO_INVALID_UTF8 && dif_eq(e.path, DIF_S("<stdin>")));
    dif_ioerr_dec(&e);
    line_is("next");
    CHECK(dif_io_read_line(&s, &e) == -1 && e.kind == DIF_IO_INVALID_UTF8); // truncated sequence at the end
    dif_ioerr_dec(&e);
    eof();
    // a very long line and an embedded NUL
    char *big = dif_alloc(300001);
    memset(big, 'y', 300000);
    big[300000] = '\n';
    feed(big, 300001);
    free(big);
    CHECK(dif_io_read_line(&s, &e) == 1 && s.n == 300000);
    dif_str_dec(s);
    FEED("a\0b\n");
    CHECK(dif_io_read_line(&s, &e) == 1 && s.n == 3 && s.p[1] == 0);
    dif_str_dec(s);
    // `for line in io.lines()` is the same call in a loop; after a break the next call goes on
    FEED("1\n2\n3\n4\n");
    int64_t total = 0, v;
    for (int r; (r = dif_io_read_line(&s, &e)) == 1;) {
        CHECK(dif_parse_int(s, &v));
        total += v;
        dif_str_dec(s);
        if (total == 3) break;
    }
    line_is("3");
    line_is("4");
    eof();
    CHECK(total == 3);
}

static void test_flush_before_read(void) {
    // _exit does not flush: the text is only there if reading flushed stdout
    FEED("x\n");
    EXPECT_EXIT(0, "prompt: ", printf("prompt: "); dif_str s; dif_ioerr e; dif_io_read_line(&s, &e));
}

static void test_os(void) {
    char *argv0[] = {"prog", NULL};
    dif_init(1, argv0);
    CHECK(dif_os_args(1, 1) == NULL);
    char *argv[] = {"prog", "a", "\xc3\xa4", "", NULL};
    dif_init(4, argv);
    dif_list *l = dif_os_args(1, 1);
    dif_buf b = dif_buf_new();
    dif_list_put(&dif_ty_str, &b, l);
    CHECK_BUF(b, "[\"a\", \"\xc3\xa4\", \"\"]");
    dif_list_dec(&dif_ty_str, l);
    char *bad[] = {"prog", "ok", "x\xff", NULL};
    dif_init(3, bad);
    EXPECT_PANIC("invalid UTF-8 in argument 1 (main.dif:2:3)", dif_os_args(2, 3));
    dif_init(1, argv0);

    dif_str v;
    CHECK(!dif_os_env(DIF_S("DIF_TEST_NO_SUCH_VARIABLE"), &v, 1, 1));
    setenv("DIF_TEST_VAR", "value \xc3\xa4", 1);
    CHECK(dif_os_env(DIF_S("DIF_TEST_VAR"), &v, 1, 1));
    CHECK_STR(v, "value \xc3\xa4");
    dif_str_dec(v);
    setenv("DIF_TEST_EMPTY", "", 1);
    CHECK(dif_os_env(DIF_S("DIF_TEST_EMPTY"), &v, 1, 1) && v.n == 0);
    CHECK(!dif_os_env(DIF_EMPTY, &v, 1, 1) && !dif_os_env(DIF_S("A=B"), &v, 1, 1));
    setenv("DIF_TEST_BAD", "\xff", 1);
    EXPECT_PANIC("invalid UTF-8 in environment variable DIF_TEST_BAD (main.dif:4:5)",
                 dif_str n = DIF_S("DIF_TEST_BAD"); dif_os_env(n, &v, 4, 5));

    EXPECT_EXIT(3, "out\n", printf("out\n"); dif_os_exit(3, 1, 1));
    EXPECT_EXIT(0, "out\n", printf("out\n"); dif_os_exit(0, 1, 1));
    EXPECT_EXIT(255, "", dif_os_exit(255, 1, 1));
    EXPECT_PANIC("exit code 256 out of range 0..255 (main.dif:7:9)", dif_os_exit(256, 7, 9));
    EXPECT_PANIC("exit code -1 out of range 0..255 (main.dif:1:1)", dif_os_exit(-1, 1, 1));
    EXPECT_EXIT(0, "e\n", fprintf(stderr, "e"); dif_eprint_str(DIF_S("")); fflush(stderr));
}

int main(void) {
    const char *t = getenv("DIF_TEST_TMP");
    snprintf(tmpdir, sizeof tmpdir, "%s", t && *t ? t : "/tmp");
    test_files();
    test_lines();
    test_flush_before_read();
    test_os();
    T_DONE();
}
