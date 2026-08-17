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

    // 数组 indexOf / includes
    const nums = [3, 1, 4, 1, 5];
    passed = passed & check(nums.indexOf(4) == 2, "indexOf hit");
    passed = passed & check(nums.indexOf(9) == -1, "indexOf miss");
    passed = passed & check(nums.includes(1), "includes hit");
    passed = passed & check(nums.includes(9) == false, "includes miss");
    const words = ["a", "b", "c"];
    passed = passed & check(words.indexOf("c") == 2, "indexOf string");
    passed = passed & check(words.includes("x") == false, "includes string miss");
    const floats = [1.5, 2.5, 3.5];
    passed = passed & check(floats.indexOf(2.5) == 1, "indexOf float");
    passed = passed & check(floats.includes(9.9) == false, "includes float miss");

    // 数组 reduce
    const sum = nums.reduce((acc: int, x: int): int => acc + x, 0);
    passed = passed & check(sum == 14, "reduce sum");
    const product = nums.reduce((acc: int, x: int): int => acc * x, 1);
    passed = passed & check(product == 60, "reduce product");
    const joined = words.reduce((acc: string, w: string): string => acc + w, "");
    passed = passed & check(joined == "abc", "reduce string");

    // shift / unshift
    const q = [1, 2, 3];
    const first = q.shift();
    passed = passed & check(first == 1 && q.length == 2, "shift");
    const nlen = q.unshift(9);
    passed = passed & check(nlen == 3 && q[0] == 9, "unshift");
    const empty = [5];
    const popped = empty.shift();
    passed = passed & check(popped == 5 && empty.length == 0, "shift to empty");
    const empty2: int[] = [];
    passed = passed & check(empty2.shift() == 0, "shift empty returns 0");

    // splice
    const arr = [10, 20, 30, 40, 50];
    const removed = arr.splice(1, 2);
    passed = passed & check(removed.length == 2 && removed[0] == 20 && removed[1] == 30, "splice removed");
    passed = passed & check(arr.length == 3 && arr[0] == 10 && arr[1] == 40 && arr[2] == 50, "splice mutate");

    // nullable 模板插值
    let count: int? = null;
    passed = passed & check(`c=${count}` == "c=null", "nullable int null");
    count = 3;
    passed = passed & check(`c=${count}` == "c=3", "nullable int value");
    let name: string? = null;
    passed = passed & check(`n=${name}` == "n=null", "nullable string null");
    name = "sw";
    passed = passed & check(`n=${name}` == "n=sw", "nullable string value");
    let ratio: float? = null;
    passed = passed & check(`r=${ratio}` == "r=null", "nullable float null");
    ratio = 0.5;
    passed = passed & check(`r=${ratio}` == "r=0.5", "nullable float value");

    // -1 as u8 优先级（Rust 语义：(-1) as u8 = 255）
    passed = passed & check((-1 as u8) == 255, "neg cast u8 255");
    passed = passed & check((-255 as i8) == 1, "neg cast i8 1");
    passed = passed & check((5 as u8) == 5, "positive cast unchanged");
    passed = passed & check((300 as u8) == 44, "300 as u8 44");
    passed = passed & check((2 * 3 as u8) == 6, "as binds tighter than mul");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
