import { println } from "std/io";

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
    const nums = [1, 2, 3, 4, 5];

    let calls = 0;
    for (const x of nums) {
        calls += 1;
    }
    passed = passed & check(calls == 5, "for-of count");
    nums.forEach((x: int) => {
        println(`item ${x}`);
    });

    passed = passed & check(nums.some((x: int) => x > 4), "some true");
    passed = passed & check(nums.some((x: int) => x > 10) == false, "some false");
    passed = passed & check(nums.every((x: int) => x > 0), "every true");
    passed = passed & check(nums.every((x: int) => x > 2) == false, "every false");
    passed = passed & check((nums.find((x: int) => x == 3) ?? 0) == 3, "find hit");
    passed = passed & check((nums.find((x: int) => x > 10) ?? 0) == 0, "find miss default");

    const stack = [10, 20];
    const pushed = stack.push(30);
    passed = passed & check(pushed == 3 && stack.length == 3 && stack[2] == 30, "push");
    const popped = stack.pop();
    passed = passed & check(popped == 30 && stack.length == 2, "pop");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
