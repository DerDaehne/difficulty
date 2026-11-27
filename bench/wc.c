// Word count, C11: a word is a run of ASCII letters; lower-cased; top 20.
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct { const char *p; size_t n; int64_t count; } Entry;

static int better(const Entry *a, const Entry *b) {
    if (a->count != b->count) return a->count > b->count ? -1 : 1;
    size_t m = a->n < b->n ? a->n : b->n;
    int c = memcmp(a->p, b->p, m);
    if (c) return c;
    return a->n < b->n ? -1 : a->n > b->n;
}
static int cmp(const void *a, const void *b) { return better(a, b); }

int main(void) {
    size_t cap = 1 << 20, len = 0, got;
    char *buf = malloc(cap);
    while ((got = fread(buf + len, 1, cap - len, stdin)) > 0) {
        len += got;
        if (len == cap) buf = realloc(buf, cap *= 2);
    }
    size_t tcap = 1 << 16, distinct = 0;
    Entry *tab = calloc(tcap, sizeof *tab);
    int64_t total = 0;
    for (size_t i = 0; i < len;) {
        size_t s = i;
        uint64_t h = 1469598103934665603ULL;
        while (i < len && ((buf[i] | 32) >= 'a' && (buf[i] | 32) <= 'z') && (unsigned char)buf[i] < 128) {
            buf[i] |= 32;
            h = (h ^ (unsigned char)buf[i]) * 1099511628211ULL;
            i++;
        }
        if (i == s) { i++; continue; }
        size_t n = i - s;
        total++;
        if (distinct * 2 >= tcap) {  // grow
            Entry *old = tab; size_t oc = tcap;
            tcap *= 2;
            tab = calloc(tcap, sizeof *tab);
            for (size_t k = 0; k < oc; k++) if (old[k].p) {
                uint64_t g = 1469598103934665603ULL;
                for (size_t j = 0; j < old[k].n; j++) g = (g ^ (unsigned char)old[k].p[j]) * 1099511628211ULL;
                size_t x = g & (tcap - 1);
                while (tab[x].p) x = (x + 1) & (tcap - 1);
                tab[x] = old[k];
            }
            free(old);
        }
        size_t x = h & (tcap - 1);
        while (tab[x].p && !(tab[x].n == n && memcmp(tab[x].p, buf + s, n) == 0)) x = (x + 1) & (tcap - 1);
        if (!tab[x].p) { tab[x].p = buf + s; tab[x].n = n; distinct++; }
        tab[x].count++;
    }
    Entry *all = malloc(distinct * sizeof *all);
    size_t k = 0;
    for (size_t x = 0; x < tcap; x++) if (tab[x].p) all[k++] = tab[x];
    qsort(all, distinct, sizeof *all, cmp);
    for (size_t j = 0; j < distinct && j < 20; j++)
        printf("%.*s %lld\n", (int)all[j].n, all[j].p, (long long)all[j].count);
    printf("total %lld distinct %zu\n", (long long)total, distinct);
    return 0;
}
