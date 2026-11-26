// Minimal test helpers for the runtime tests: CHECK macros and abort tests in a child process.
#define _POSIX_C_SOURCE 200809L
#include <fcntl.h>
#include <sys/wait.h>
#include <unistd.h>

#define DIF_EXIT _exit // a forked child must not run the leak check after a panic
static const char *const dif_file = "main.dif";
#include "dif_rt.h"

static int t_fail = 0, t_count = 0;

#define CHECK(cond)                                                                      \
    do {                                                                                 \
        t_count++;                                                                       \
        if (!(cond)) {                                                                   \
            fprintf(stderr, "%s:%d: CHECK failed: %s\n", __FILE__, __LINE__, #cond);     \
            t_fail++;                                                                    \
        }                                                                                \
    } while (0)

// compare a dif_str with a C string
#define CHECK_STR(s, lit)                                                                \
    do {                                                                                 \
        dif_str t_s = (s);                                                               \
        t_count++;                                                                       \
        if (t_s.n != (int64_t)strlen(lit) || memcmp(t_s.p, lit, (size_t)t_s.n) != 0) {   \
            fprintf(stderr, "%s:%d: expected \"%s\", got \"%.*s\"\n", __FILE__, __LINE__, \
                    lit, (int)t_s.n, t_s.p);                                             \
            t_fail++;                                                                    \
        }                                                                                \
    } while (0)

// Take the text of a builder, compare it with a C string and release it.
#define CHECK_BUF(b, lit)                                                                \
    do {                                                                                 \
        dif_str t_b = dif_take(&(b));                                                    \
        CHECK_STR(t_b, lit);                                                             \
        dif_str_dec(t_b);                                                                \
    } while (0)

static int t_pipe[2];
// Runs the statements in a child whose stdout and stderr go to one pipe; the child is
// expected to abort with exit code 2 and exactly `expected` as output.
__attribute__((unused)) static int t_child_begin(void) {
    fflush(stdout);
    fflush(stderr);
    if (pipe(t_pipe) != 0) abort();
    pid_t pid = fork();
    if (pid == 0) {
        dup2(t_pipe[1], 1);
        dup2(t_pipe[1], 2);
        close(t_pipe[0]);
        close(t_pipe[1]);
        return 0;
    }
    close(t_pipe[1]);
    return (int)pid;
}
__attribute__((unused)) static void t_child_end(int pid, const char *expected, int code, const char *file, int line) {
    char out[4096];
    size_t n = 0;
    ssize_t r;
    while (n < sizeof out - 1 && (r = read(t_pipe[0], out + n, sizeof out - 1 - n)) > 0) n += (size_t)r;
    out[n] = 0;
    char *o = out; // ASan may print a warning line before ours
    while (strncmp(o, "==", 2) == 0 && strchr(o, '\n')) o = strchr(o, '\n') + 1;
    close(t_pipe[0]);
    int st = 0;
    waitpid(pid, &st, 0);
    t_count++;
    if (!WIFEXITED(st) || WEXITSTATUS(st) != code || strcmp(o, expected) != 0) {
        fprintf(stderr, "%s:%d: child: exit %d, output \"%s\"; expected exit %d, \"%s\"\n", file, line,
                WIFEXITED(st) ? WEXITSTATUS(st) : -1, o, code, expected);
        t_fail++;
    }
}
// EXPECT_PANIC("msg (main.dif:3:4)", statements...)
#define EXPECT_PANIC(msg, ...)                                                           \
    do {                                                                                 \
        int t_pid = t_child_begin();                                                     \
        if (t_pid == 0) {                                                                \
            __VA_ARGS__;                                                                 \
            _exit(0);                                                                    \
        }                                                                                \
        t_child_end(t_pid, "panic: " msg "\n", 2, __FILE__, __LINE__);                   \
    } while (0)
// like EXPECT_PANIC for any exit code and complete output (stdout before the message)
#define EXPECT_EXIT(code, out, ...)                                                      \
    do {                                                                                 \
        int t_pid = t_child_begin();                                                     \
        if (t_pid == 0) {                                                                \
            __VA_ARGS__;                                                                 \
            _exit(0);                                                                    \
        }                                                                                \
        t_child_end(t_pid, out, code, __FILE__, __LINE__);                               \
    } while (0)

#define T_DONE()                                                                         \
    do {                                                                                 \
        if (t_fail) {                                                                    \
            fprintf(stderr, "%s: %d of %d checks failed\n", __FILE__, t_fail, t_count);  \
            return 1;                                                                    \
        }                                                                                \
        printf("%s: %d checks ok\n", __FILE__, t_count);                                 \
        return 0;                                                                        \
    } while (0)

// ASan: an oversized allocation returns NULL (the runtime then reports out of memory)
const char *__asan_default_options(void) { return "allocator_may_return_null=1"; }
