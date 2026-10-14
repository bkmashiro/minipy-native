// Fixed Quote/v1 checked weighted/stateful scan. SysV x86_64 ABI:
// rdi rows, rsi count, rdx total*, rcx previous*, r8 limit, r9 stateful.
// Returns eax=0 success / 1 signed overflow, commits state only on success.
// Caller-saved GPRs only, no stack, calls, syscalls or Rust arithmetic helper.
.text
.p2align 4
.global minipy_l_scan
.type minipy_l_scan, @function
minipy_l_scan:
    mov r10, [rdx]
    mov r11, [rcx]
    test rsi, rsi
    jz .Lfinish
    test r9, r9
    jz .Lweighted
.Lstateful:
    mov rax, [rdi]
    mov r9, [rdi + 8]
    test r9, r9
    jle .Lstateful_next
    cmp rax, r8
    jg .Lstateful_next
    cmp rax, r11
    jl .Lstateful_next
    imul r9, rax
    jo .Loverflow
    add r10, r9
    jo .Loverflow
    mov r11, rax
.Lstateful_next:
    add rdi, 16
    dec rsi
    jnz .Lstateful
    jmp .Lfinish
.Lweighted:
    mov rax, [rdi]
    mov r9, [rdi + 8]
    test r9, r9
    jle .Lweighted_next
    cmp rax, r8
    jg .Lweighted_next
    imul r9, rax
    jo .Loverflow
    add r10, r9
    jo .Loverflow
.Lweighted_next:
    add rdi, 16
    dec rsi
    jnz .Lweighted
.Lfinish:
    mov [rdx], r10
    mov [rcx], r11
    xor eax, eax
    ret
.Loverflow:
    mov eax, 1
    ret
.global minipy_l_scan_end
minipy_l_scan_end:
.size minipy_l_scan, .-minipy_l_scan
.section .note.GNU-stack,"",@progbits
