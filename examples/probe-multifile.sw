import { println } from "std/io";
import { Point, Vector, make_point } from "./lib-geo";

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
    const p: Point = { x: 1, y: 2.5 };
    ok = check(ok, p.x == 1, "cross_file_field_x");
    ok = check(ok, p.y == 2.5, "cross_file_field_y");
    const q = make_point(3, 4.5);
    ok = check(ok, q.x == 3, "cross_file_fn_x");
    ok = check(ok, q.y == 4.5, "cross_file_fn_y");
    ok = check(ok, p == make_point(1, 2.5), "cross_file_struct_eq");
    const v = new Vector(2, 3);
    ok = check(ok, v.dx == 2, "cross_file_class_field");
    ok = check(ok, v.magnitude() == 5, "cross_file_class_method");
    println(`final=${ok == 1 ? "PASS" : "FAIL"}`);
    return ok == 1 ? 0 : 1;
}
