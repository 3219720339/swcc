// aarch64 setjmp/longjmp（自包含，不依赖 CRT）
// sw_setjmp(buf)：x0 = buf，保存被调用者保存寄存器 + SP/LR，返回 0
// sw_longjmp(buf, value)：x0 = buf，w1 = value，恢复并跳回 sw_setjmp 调用点
// 同时导出带下划线的 Darwin 别名（Mach-O C ABI），ELF/COFF 上无害。
    .text
    .globl sw_setjmp
    .globl _sw_setjmp
    .globl sw_longjmp
    .globl _sw_longjmp

sw_setjmp:
_sw_setjmp:
    stp x19, x20, [x0, #0x00]
    stp x21, x22, [x0, #0x10]
    stp x23, x24, [x0, #0x20]
    stp x25, x26, [x0, #0x30]
    stp x27, x28, [x0, #0x40]
    stp x29, x30, [x0, #0x50]
    mov x9, sp
    str x9, [x0, #0x60]
    stp d8, d9, [x0, #0x70]
    stp d10, d11, [x0, #0x80]
    stp d12, d13, [x0, #0x90]
    stp d14, d15, [x0, #0xa0]
    mov w0, #0
    ret

sw_longjmp:
_sw_longjmp:
    ldp x19, x20, [x0, #0x00]
    ldp x21, x22, [x0, #0x10]
    ldp x23, x24, [x0, #0x20]
    ldp x25, x26, [x0, #0x30]
    ldp x27, x28, [x0, #0x40]
    ldp x29, x30, [x0, #0x50]
    ldr x9, [x0, #0x60]
    mov sp, x9
    ldp d8, d9, [x0, #0x70]
    ldp d10, d11, [x0, #0x80]
    ldp d12, d13, [x0, #0x90]
    ldp d14, d15, [x0, #0xa0]
    mov w0, w1
    ret
