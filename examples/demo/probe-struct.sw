import { println } from "std/io";

struct Point {
    x: int;
    y: float;
}

function make(): Point {
    const p: Point = { x: 3, y: 4.5 };
    return p;
}

function sum(p: Point): float {
    return p.x as float + p.y;
}

function main(): int {
    const p = make();
    const total = sum(p);
    let q: Point = { x: 1, y: 2.0 };
    q.x = 9;
    const copy = q;
    q.x = 100;
    let n = 1;
    const old = n++;
    const fresh = ++n;
    let a = 5;
    const assigned = (a = 10);
    const power = 2 ** 10;
    const fpower = 2.0 ** 3.0;
    println(`total=${total} q=${q.x} copy=${copy.x} old=${old} fresh=${fresh} a=${a} assigned=${assigned} power=${power} fpower=${fpower}`);
    return 0;
}
