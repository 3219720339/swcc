import { println } from "std/io";

class Utils {
    static version: int;
    static enabled: bool;
    // 带初始值的静态字段（bug #4 修复：此前初值被丢弃，读到零值；
    // float 类型还导致 codegen IR 校验失败）。
    static VERSION: int = 3;
    static RATIO: float = 2.5;
    static FLAG: bool = true;
    static COUNT: int = -7;

    static make_label(prefix: string, value: int): string {
        return prefix + value;
    }

    static triple(x: int): int {
        return x * 3;
    }
}

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
    passed = passed & check(Utils.triple(5) == 15, "static method call");
    passed = passed & check(Utils.make_label("v", 7) == "v7", "static method with args");
    passed = passed & check(Utils.version == 0, "static field default");
    passed = passed & check(Utils.enabled == false, "static bool field default");
    // 带初值的静态字段（bug #4 回归）
    passed = passed & check(Utils.VERSION == 3, "static int field with initializer");
    passed = passed & check(Utils.RATIO == 2.5, "static float field with initializer");
    passed = passed & check(Utils.FLAG == true, "static bool field with initializer");
    passed = passed & check(Utils.COUNT == -7, "static negative int field with initializer");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
