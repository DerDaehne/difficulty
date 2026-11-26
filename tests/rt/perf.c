// Timing tests, run only optimized and without sanitizers (PLAN 10): 1 000 000 pushes < 50 ms.
#include "t.h"
#include <time.h>

static double now_ms(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (double)ts.tv_sec * 1e3 + (double)ts.tv_nsec / 1e6;
}

static double push_million(void) {
    double t0 = now_ms();
    dif_list *l = NULL;
    for (int64_t i = 0; i < 1000000; i++) dif_list_push(&dif_ty_int, &l, &i);
    double t = now_ms() - t0;
    CHECK(dif_list_len(l) == 1000000 && ((int64_t *)dif_list_data(l))[999999] == 999999);
    dif_list_dec(&dif_ty_int, l);
    return t;
}

int main(void) {
    double best = 1e9;
    for (int i = 0; i < 5; i++) { // best of five: a loaded machine must not fail the test
        double t = push_million();
        if (t < best) best = t;
    }
    printf("1000000 x push: %.1f ms\n", best);
    CHECK(best < 50.0);
    T_DONE();
}
