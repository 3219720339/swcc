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
    const doubled = numbers.map((x: int) => x * 2);
    const evens = numbers.filter((x: int) => x % 2 == 0);
    const chained = numbers
        .filter((x: int) => x > 2)
        .map((x: int) => x * 10);

    passed = passed & check(doubled.length == 5, "map length");
    passed = passed & check(doubled[0] == 2 && doubled[4] == 10, "map values");
    passed = passed & check(sum_int(doubled) == 30, "map sum");
    passed = passed & check(evens.length == 2 && evens[0] == 2 && evens[1] == 4, "filter values");
    passed = passed & check(chained.length == 3 && chained[0] == 30 && chained[2] == 50, "chained filter map");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
