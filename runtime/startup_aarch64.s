// 极简 aarch64 启动：mainCRTStartup -> main -> exit
    .text
    .globl mainCRTStartup
mainCRTStartup:
    stp x29, x30, [sp, #-16]!
    bl main
    bl exit
    brk #0
