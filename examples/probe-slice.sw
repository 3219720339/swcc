import { println } from "std/io";
import { sum_int } from "std/array";

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
    const numbers = [1, 2, 3, 4, 5];
    const mid = numbers[1:3];
    const head = numbers[:2];
    const tail = numbers[2:];
    const empty = numbers[3:2];

    passed = passed & check(mid.length == 2 && mid[0] == 2 && mid[1] == 3, "slice a[1:3]");
    passed = passed & check(head.length == 2 && head[1] == 2, "slice a[:2]");
    passed = passed & check(tail.length == 3 && tail[0] == 3 && tail[2] == 5, "slice a[2:]");
    passed = passed & check(empty.length == 0, "empty slice");
    passed = passed & check(sum_int(mid) == 5, "slice sum");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
