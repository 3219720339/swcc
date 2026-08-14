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
    // u8[] 紧凑布局：切片必须按 1 字节步长复制，否则数据错乱。
    const bytes = [1u8, 2u8, 3u8, 4u8, 5u8, 6u8];
    const mid = bytes[1:4];   // [2,3,4]
    const head = bytes[:2];   // [1,2]
    const tail = bytes[3:];   // [4,5,6]
    passed = passed & check(mid.length == 3 && mid[0] == 2 && mid[1] == 3 && mid[2] == 4, "u8 slice mid [2,3,4]");
    passed = passed & check(head.length == 2 && head[1] == 2, "u8 slice head [:2]");
    passed = passed & check(tail.length == 3 && tail[0] == 4 && tail[2] == 6, "u8 slice tail [3:]");
    passed = passed & check(bytes[0] == 1 && bytes[5] == 6 && bytes.length == 6, "u8 original intact");

    // 常规 int[]（8 字节步长）切片不受影响。
    const nums = [10, 20, 30, 40];
    const nums_mid = nums[1:3];
    passed = passed & check(nums_mid.length == 2 && nums_mid[0] == 20 && nums_mid[1] == 30, "int slice still works");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
