// 崩溃定位验证探针：数组越界触发崩溃，验证崩溃处理器打印
// 「函数名 (文件:行号)」调用栈。
// 注意：本探针预期退出码为 3（崩溃处理器 exit(3)），不注册 run-examples。
import { println, flush } from "std/io";

function crash_site(): int {
    const arr = [1, 2, 3, 4, 5];
    const idx = 1000000000;  // 越界 → 访问违规崩溃
    return arr[idx];
}

function middle(): int {
    return crash_site();
}

function outer(): int {
    return middle();
}

function main(): int {
    println("before crash");
    flush();
    const r = outer();
    println(`never: ${r}`);
    return 0;
}
