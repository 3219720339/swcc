import { println, pause, 暂停 } from "std/io";

function main(): int {
    println("中文输出测试：你好，Sw 语言");
    println("程序将暂停，按任意键继续...");
    pause();
    println("继续运行，再次暂停（中文函数名）");
    暂停();
    println("结束");
    return 0;
}
