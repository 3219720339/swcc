// 进程参数引号负例：run()/spawn() 的 CreateProcessA 命令行编码（sw_build_cmdline）
// 必须按 Windows 反斜杠/引号规则处理，参数含空格、引号、尾随反斜杠、空串时
// 子进程收到的 argv 与传入完全一致（POSIX execvp 数组直传天然正确，本探针
// 同时验证运行时 argv 构建往返）。
import { println } from "std/io";
import { run } from "std/os";

function check(condition: bool, label: string): int {
    if (condition) {
        println(`[ok] ${label}`);
        return 1;
    }
    println(`[FAIL] ${label}`);
    return 0;
}

function main(args: string[]): int {
    // 子进程模式：精确回显 argv（每参数一行，[] 包裹便于父进程精确匹配）。
    if (args.length > 1 && args[1] == "--echo") {
        let i = 2;
        while (i < args.length) {
            println(`[${args[i]}]`);
            i = i + 1;
        }
        return 0;
    }

    // 父进程模式：用 run() 调自身，传含特殊字符的参数。
    let passed = 1;
    const self = args[0];
    const out = run(self, ["--echo", "a b", "c\"d", "e\\", ""]);

    passed = passed & check(out.index_of("[a b]") >= 0, "arg with space");
    passed = passed & check(out.index_of("[c\"d]") >= 0, "arg with quote");
    passed = passed & check(out.index_of("[e\\]") >= 0, "arg trailing backslash");
    passed = passed & check(out.index_of("[]") >= 0, "arg empty string");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
