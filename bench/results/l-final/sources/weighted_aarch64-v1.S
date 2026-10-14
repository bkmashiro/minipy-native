// Fixed Quote/v1 checked filter/product/sum, optional monotonic-price state.
// AAPCS64: x0 rows, x1 count, x2 total*, x3 previous*, x4 limit, x5 stateful.
// Returns x0=0 success / 1 signed overflow. Commits scalar state on success.
// Clobbers only x0,x1,x6-x11 and NZCV; no stack, allocation or syscall.
.text
.p2align 2
.global minipy_l_scan
.type minipy_l_scan, %function
minipy_l_scan:
    ldr x6, [x2]
    ldr x7, [x3]
    cbz x1, .Lfinish
.Lrow:
    ldp x8, x9, [x0], #16
    cmp x9, #0
    b.le .Lnext
    cmp x8, x4
    b.gt .Lnext
    cbz x5, .Lproduct
    cmp x8, x7
    b.lt .Lnext
.Lproduct:
    mul x10, x8, x9
    smulh x11, x8, x9
    cmp x11, x10, asr #63
    b.ne .Loverflow
    adds x6, x6, x10
    b.vs .Loverflow
    cbz x5, .Lnext
    mov x7, x8
.Lnext:
    subs x1, x1, #1
    b.ne .Lrow
.Lfinish:
    str x6, [x2]
    str x7, [x3]
    mov x0, #0
    ret
.Loverflow:
    mov x0, #1
    ret
.global minipy_l_scan_end
minipy_l_scan_end:
.size minipy_l_scan, .-minipy_l_scan
.section .note.GNU-stack,"",%progbits
