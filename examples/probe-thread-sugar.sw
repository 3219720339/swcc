// 线程传参/回传糖 + 闭包捕获修复验证。
import { println } from "std/io";
import { thread_spawn, spawn_with, spawn_result, thread_join, thread_result } from "std/thread";

class Box { v: int; constructor(x: int) { this.v = x; } }

function double(x: int): int { return x * 2; }
function add(a: int, b: int): int { return a + b; }

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

    // spawn_with：带参任务（底层闭包值捕获）
    const t1 = spawn_with(double, 21);
    thread_join(t1, -1);
    passed = passed & check(thread_result(t1) == 42, "spawn_with arg");

    // 多参：手动捕获
    const x = 3;
    const y = 4;
    const t2 = thread_spawn((): int => add(x, y));
    thread_join(t2, -1);
    passed = passed & check(thread_result(t2) == 7, "multi-arg via capture");

    // spawn_result：复杂结果（string）回传 out[0]
    const out: string[] = [""];
    const t3 = spawn_result((s: string): string => s + "!", "hi", out);
    thread_join(t3, -1);
    passed = passed & check(out[0] == "hi!", "spawn_result string");

    // spawn_result：class 结果回传
    const objs: Box[] = [new Box(0)];
    const t4 = spawn_result((n: int): Box => new Box(n), 7, objs);
    thread_join(t4, -1);
    passed = passed & check(objs[0].v == 7, "spawn_result class field");

    // P0 回归：闭包内 class/struct 字段读写
    const b = new Box(1);
    const cl = (): int => {
        b.v = 99;
        return b.v;
    };
    passed = passed & check(cl() == 99 && b.v == 99, "closure class field write");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
