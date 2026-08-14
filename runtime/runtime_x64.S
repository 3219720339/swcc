// x64 自包含 setjmp/longjmp（不依赖 CRT）
// sw_setjmp(buf): 保存被调用者保存寄存器与 RSP/RIP，返回 0
// sw_longjmp(buf, value): 恢复并跳回 sw_setjmp 调用点，返回 value
//
// 调用约定按目标平台选择：
//   Windows x64（COFF）：第一个参数 rcx，第二个 rdx
//   SysV（ELF / Mach-O）：第一个参数 rdi，第二个 rsi
#if defined(_WIN32)
#define SW_BUF rcx
#define SW_VAL edx
#else
#define SW_BUF rdi
#define SW_VAL esi
#endif
    .text
    .globl sw_setjmp
    .globl _sw_setjmp
    .globl sw_longjmp
    .globl _sw_longjmp

sw_setjmp:
_sw_setjmp:
    // %SW_BUF = buf
    movq %rbx, 0x00(%SW_BUF)
    movq %rbp, 0x08(%SW_BUF)
    movq %rsi, 0x10(%SW_BUF)
    movq %rdi, 0x18(%SW_BUF)
    movq %r12, 0x20(%SW_BUF)
    movq %r13, 0x28(%SW_BUF)
    movq %r14, 0x30(%SW_BUF)
    movq %r15, 0x38(%SW_BUF)
    movq (%rsp), %rax
    movq %rax, 0x48(%SW_BUF)
    leaq 8(%rsp), %rax
    movq %rax, 0x40(%SW_BUF)
    movaps %xmm6, 0x50(%SW_BUF)
    movaps %xmm7, 0x60(%SW_BUF)
    movaps %xmm8, 0x70(%SW_BUF)
    movaps %xmm9, 0x80(%SW_BUF)
    movaps %xmm10, 0x90(%SW_BUF)
    movaps %xmm11, 0xa0(%SW_BUF)
    movaps %xmm12, 0xb0(%SW_BUF)
    movaps %xmm13, 0xc0(%SW_BUF)
    movaps %xmm14, 0xd0(%SW_BUF)
    movaps %xmm15, 0xe0(%SW_BUF)
    xorl %eax, %eax
    ret

sw_longjmp:
_sw_longjmp:
    // %SW_BUF = buf, %SW_VAL = value
    movq 0x00(%SW_BUF), %rbx
    movq 0x08(%SW_BUF), %rbp
    movq 0x10(%SW_BUF), %rsi
    movq 0x18(%SW_BUF), %rdi
    movq 0x20(%SW_BUF), %r12
    movq 0x28(%SW_BUF), %r13
    movq 0x30(%SW_BUF), %r14
    movq 0x38(%SW_BUF), %r15
    movaps 0x50(%SW_BUF), %xmm6
    movaps 0x60(%SW_BUF), %xmm7
    movaps 0x70(%SW_BUF), %xmm8
    movaps 0x80(%SW_BUF), %xmm9
    movaps 0x90(%SW_BUF), %xmm10
    movaps 0xa0(%SW_BUF), %xmm11
    movaps 0xb0(%SW_BUF), %xmm12
    movaps 0xc0(%SW_BUF), %xmm13
    movaps 0xd0(%SW_BUF), %xmm14
    movaps 0xe0(%SW_BUF), %xmm15
    movq 0x48(%SW_BUF), %r8
    movq 0x40(%SW_BUF), %rsp
    movl %SW_VAL, %eax
    pushq %r8
    ret
