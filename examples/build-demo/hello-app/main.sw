// 控制台程序演示：依赖静态库导出（此处直接内联，便于独立运行）。
import { println } from "std/io";

function add(a: int, b: int): int {
    return a + b;
}

function main(): int {
    const a = 20;
    const b = 22;
    println(`hello from swcc: ${a} + ${b} = ${add(a, b)}`);
    println(`square(7) = ${7 * 7}`);
    return 0;
}
