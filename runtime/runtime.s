// x64 自包含 setjmp/longjmp（不依赖 CRT）
// sw_setjmp(buf): 保存被调用者保存寄存器与 RSP/RIP，返回 0
// sw_longjmp(buf, value): 恢复并跳回 sw_setjmp 调用点，返回 value
    .text
    .globl sw_setjmp
    .globl sw_longjmp

sw_setjmp:
    // rcx = buf
    movq %rbx, 0x00(%rcx)
    movq %rbp, 0x08(%rcx)
    movq %rsi, 0x10(%rcx)
    movq %rdi, 0x18(%rcx)
    movq %r12, 0x20(%rcx)
    movq %r13, 0x28(%rcx)
    movq %r14, 0x30(%rcx)
    movq %r15, 0x38(%rcx)
    movq (%rsp), %rax
    movq %rax, 0x48(%rcx)
    leaq 8(%rsp), %rax
    movq %rax, 0x40(%rcx)
    movaps %xmm6, 0x50(%rcx)
    movaps %xmm7, 0x60(%rcx)
    movaps %xmm8, 0x70(%rcx)
    movaps %xmm9, 0x80(%rcx)
    movaps %xmm10, 0x90(%rcx)
    movaps %xmm11, 0xa0(%rcx)
    movaps %xmm12, 0xb0(%rcx)
    movaps %xmm13, 0xc0(%rcx)
    movaps %xmm14, 0xd0(%rcx)
    movaps %xmm15, 0xe0(%rcx)
    xorl %eax, %eax
    ret

sw_longjmp:
    // rcx = buf, rdx = value
    movq 0x00(%rcx), %rbx
    movq 0x08(%rcx), %rbp
    movq 0x10(%rcx), %rsi
    movq 0x18(%rcx), %rdi
    movq 0x20(%rcx), %r12
    movq 0x28(%rcx), %r13
    movq 0x30(%rcx), %r14
    movq 0x38(%rcx), %r15
    movaps 0x50(%rcx), %xmm6
    movaps 0x60(%rcx), %xmm7
    movaps 0x70(%rcx), %xmm8
    movaps 0x80(%rcx), %xmm9
    movaps 0x90(%rcx), %xmm10
    movaps 0xa0(%rcx), %xmm11
    movaps 0xb0(%rcx), %xmm12
    movaps 0xc0(%rcx), %xmm13
    movaps 0xd0(%rcx), %xmm14
    movaps 0xe0(%rcx), %xmm15
    movq 0x48(%rcx), %r8
    movq 0x40(%rcx), %rsp
    movl %edx, %eax
    pushq %r8
    ret
