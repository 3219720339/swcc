import { println } from "std/io";

struct Point {
    x: int;
    y: float;
    name: string;
}

class Holder {
    p: Point;
    extra: int;

    constructor(p: Point, extra: int) {
        this.p = p;
        this.extra = extra;
    }

    get_x(): int {
        return this.p.x;
    }

    set_p(p: Point): void {
        this.p = p;
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

    // 类字段存 struct
    const p1: Point = { x: 1, y: 2.5, name: "one" };
    const h = new Holder(p1, 9);
    ok = check(ok, h.p.x == 1, "class_field_x");
    ok = check(ok, h.p.name == "one", "class_field_name");
    ok = check(ok, h.get_x() == 1, "class_method_field");
    const p2: Point = { x: 7, y: 8.5, name: "two" };
    h.set_p(p2);
    ok = check(ok, h.p.x == 7, "class_field_assign");
    ok = check(ok, h.extra == 9, "class_other_field");

    // struct 数组
    const p3: Point = { x: 3, y: 4.5, name: "three" };
    const arr = [p1, p3];
    ok = check(ok, arr.length == 2, "struct_array_len");
    ok = check(ok, arr[0].x == 1, "struct_array_index_x");
    ok = check(ok, arr[1].name == "three", "struct_array_index_name");
    const copied = arr[0];
    ok = check(ok, copied == p1, "struct_array_copy_eq");
    arr[1] = p2;
    ok = check(ok, arr[1].x == 7, "struct_array_assign");
    let count = 0;
    for (const item of arr) {
        count = count + item.x;
    }
    ok = check(ok, count == 8, "struct_array_forof");

    println(`final=${ok == 1 ? "PASS" : "FAIL"}`);
    return ok == 1 ? 0 : 1;
}
