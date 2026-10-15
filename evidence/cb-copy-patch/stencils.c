// Single-target feasibility input, not a source compiler or production backend.
// All operands are trusted runtime-owned i64 slots. No guest pointers or helpers.
// arm64 Darwin C ABI: x0 is state, w0 is status on return. Leaf tail linkage.
typedef long long i64;
extern int next(i64 *);
int add(i64 *s) {
    i64 value;
    if (__builtin_add_overflow(s[1], s[2], &value)) return 1;
    s[3] = value;
    __attribute__((musttail)) return next(s);
}
int sub(i64 *s) {
    i64 value;
    if (__builtin_sub_overflow(s[1], s[2], &value)) return 1;
    s[3] = value;
    __attribute__((musttail)) return next(s);
}
int mul(i64 *s) {
    i64 value;
    if (__builtin_mul_overflow(s[1], s[2], &value)) return 1;
    s[3] = value;
    __attribute__((musttail)) return next(s);
}
int done(i64 *s) {
    (void)s;
    return 0;
}
