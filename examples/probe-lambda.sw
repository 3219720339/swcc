// 带返回类型注解的 lambda（bug #3 修复）：`(x: int): int => ...`
// 此前解析器不接受 `): int =>`（EBNF 缺返回类型），文档示例编译不过。
// 现在支持表达式体与块体，且返回类型不匹配时编译期报错。
import { println } from "std/io";

function check(condition: bool, label: string): int {
    if (condition) {
        println(`[ok] ${label}`);
        return 1;
    }
    println(`[FAIL] ${label}`);
    return 0;
}

function main(): int {
    let passed = 1;
    const double = (x: int): int => x * 2;
    const inc = (): int => { return 5; };
    const add = (a: int, b: int): int => a + b;
    const greet = (name: string): string => "hi " + name;
    const neg = (x: int): int => -x;

    passed = passed & check(double(4) == 8, "expr body with ret type");
    passed = passed & check(inc() == 5, "block body with ret type");
    passed = passed & check(add(2, 3) == 5, "multi param with ret type");
    passed = passed & check(greet("sw") == "hi sw", "string ret type");
    passed = passed & check(neg(7) == -7, "unary minus ret type");

    // 无注解 lambda 不回归
    const plain = (x: int) => x + 1;
    const block_plain = () => { return 42; };
    passed = passed & check(plain(1) == 2, "unannotated expr body");
    passed = passed & check(block_plain() == 42, "unannotated block body");

    // 闭包捕获 + 返回类型注解
    const base = 10;
    const offset = (x: int): int => x + base;
    passed = passed & check(offset(5) == 15, "capture with ret type");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
