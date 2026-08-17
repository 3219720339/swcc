import { println } from "std/io";

function double(x: int): int { return x * 2; }
function triple(x: int): int { return x * 3; }

// A: 函数类型注解——参数
function apply(f: (int) => int, x: int): int {
    return f(x);
}

// A: 多参数函数类型 + 返回函数类型
function combine(f: (int, int) => int, a: int, b: int): int {
    return f(a, b);
}
function make_adder(n: int): (int) => int {
    return (x: int): int => x + n;
}

// A: 空参数函数类型
function run_once(f: () => int): int {
    return f();
}

// C: struct 字段存回调
struct Handler {
    cb: (int) => int;
}

// C: class 字段存回调
class Machine {
    cb: (int) => int;
    constructor(c: (int) => int) { this.cb = c; }
    run(x: int): int { return this.cb(x); }
}

function check(condition: bool, label: string): int {
    if (condition) {
        println(`[ok] ${label}`);
        return 1;
    }
    println(`[FAIL] ${label}`);
    return 0;
}

function add(a: int, b: int): int { return a + b; }

function main(): int {
    let passed = 1;

    // A: 闭包传参
    passed = passed & check(apply((x: int): int => x * 10, 5) == 50, "fn param lambda");
    // B: 具名函数作为值传参
    passed = passed & check(apply(double, 5) == 10, "fn param named");
    passed = passed & check(apply(triple, 4) == 12, "fn param named 2");
    // B: 具名函数作为值赋变量 + 调用
    const f = double;
    passed = passed & check(f(21) == 42, "named fn to var");
    // B: map 传具名函数
    const nums = [1, 2, 3];
    const doubled = nums.map(double);
    passed = passed & check(doubled[0] == 2 && doubled[2] == 6, "map named fn");
    // A: 多参数 + 返回函数类型
    passed = passed & check(combine(add, 3, 4) == 7, "multi-param fn type");
    const adder = make_adder(5);
    passed = passed & check(adder(3) == 8, "return closure");
    // A: 空参数函数类型
    passed = passed & check(run_once((): int => 42) == 42, "empty-param fn type");

    // C: struct 字段回调
    const h: Handler = { cb: double };
    passed = passed & check(h.cb(3) == 6, "struct field callback");
    // C: class 字段回调（构造传参 + 调用）
    const m = new Machine(triple);
    passed = passed & check(m.run(4) == 12, "class field callback");
    // C: class 字段回调赋值
    const m2 = new Machine(double);
    m2.cb = triple;
    passed = passed & check(m2.run(2) == 6, "class field callback assign");
    // C: 回调数组
    const fns: ((int) => int)[] = [double, triple];
    passed = passed & check(fns[0](5) == 10 && fns[1](5) == 15, "callback array");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
