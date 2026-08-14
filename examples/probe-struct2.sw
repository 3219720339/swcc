import { println } from "std/io";

struct Point {
    x: int;
    y: float;
    name: string;
}

struct Inner {
    p: Point;
    tag: int;
}

interface Shape {
    center(): Point;
}

class Box implements Shape {
    px: int;
    py: float;
    pname: string;

    constructor(p: Point) {
        this.px = p.x;
        this.py = p.y;
        this.pname = p.name;
    }

    center(): Point {
        const r: Point = { x: this.px, y: this.py, name: this.pname };
        return r;
    }
}

function check(prev: int, cond: bool, label: string): int {
    let state = "FAIL";
    if (cond) {
        state = "ok";
    }
    println(`[${state}] ${label}`);
    if (cond) {
        return prev;
    }
    return 0;
}

function main(): int {
    let ok = 1;

    // struct 相等比较
    const a: Point = { x: 1, y: 2.5, name: "a" };
    const b: Point = { x: 1, y: 2.5, name: "a" };
    const c: Point = { x: 1, y: 2.5, name: "b" };
    ok = check(ok, a == b, "struct_eq");
    ok = check(ok, a != c, "struct_ne");
    const lit: Point = { x: 1, y: 2.5, name: "a" };
    ok = check(ok, a == lit, "struct_eq_literal");
    const inner1: Inner = { p: a, tag: 7 };
    const inner2: Inner = { p: b, tag: 7 };
    const inner3: Inner = { p: c, tag: 7 };
    ok = check(ok, inner1 == inner2, "struct_eq_nested");
    ok = check(ok, inner1 != inner3, "struct_ne_nested");

    // 接口方法返回 struct
    const bp: Point = { x: 3, y: 4.5, name: "box" };
    const box = new Box(bp);
    const s: Shape = box;
    const center = s.center();
    ok = check(ok, center.x == 3, "iface_ret_x");
    ok = check(ok, center.name == "box", "iface_ret_name");

    // 闭包 struct 参数
    const get_x = (p: Point) => p.x;
    ok = check(ok, get_x(a) == 1, "closure_struct_param");

    println(`final=${ok == 1 ? "PASS" : "FAIL"}`);
    return ok == 1 ? 0 : 1;
}
