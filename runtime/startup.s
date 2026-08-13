// 极简 x64 启动：mainCRTStartup → main → exit
    .text
    .globl mainCRTStartup
mainCRTStartup:
    subq $0x28, %rsp
    call main
    movl %eax, %ecx
    call exit
    hlt
