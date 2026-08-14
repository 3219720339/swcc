import { println } from "std/io";

function describe(x: int): string {
    return "int " + x;
}

function describe(x: string): string {
    return "string " + x;
}

function describe(x: int, y: int): string {
    return "pair " + x + "," + y;
}

function check(condition: bool, label: string): int {
    if (condition) {
        println(`[ok] ${label}`);
        return 1;
    }
    println(`[FAIL] ${label}`);
    return 0;
}

class Formatter {
    format(value: int): string {
        return "i" + value;
    }

    format(value: string): string {
        return "s" + value;
    }
}

function main(): int {
    let passed = 1;
    passed = passed & check(describe(1) == "int 1", "overload by type");
    passed = passed & check(describe("hi") == "string hi", "overload string");
    passed = passed & check(describe(1, 2) == "pair 1,2", "overload by arity");

    // 无显式构造函数的类也可以 new（隐式空构造）。
    const formatter = new Formatter();
    passed = passed & check(formatter.format(7) == "i7", "method overload int");
    passed = passed & check(formatter.format("x") == "sx", "method overload string");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
