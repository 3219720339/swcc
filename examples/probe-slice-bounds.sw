// 切片越界负例：越界切片必须被安全裁剪，绝不崩溃或访问原生越界内存。
// 与下标越界（exit 3）不同，切片在 runtime 是裁剪语义，本探针验证裁剪正确。
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
    const values = [10, 20, 30, 40, 50];

    // end 超过 len：裁剪到 len
    const over = values[2:99];
    passed = passed & check(over.length == 3 && over[0] == 30 && over[2] == 50, "slice end clamp");

    // start 为负：裁剪到 0
    const neg = values[-3:2];
    passed = passed & check(neg.length == 2 && neg[0] == 10 && neg[1] == 20, "slice start clamp");

    // start >= end：空数组
    const empty = values[4:2];
    passed = passed & check(empty.length == 0, "slice empty range");

    // start 超过 len：空数组
    const far = values[99:];
    passed = passed & check(far.length == 0, "slice far start");

    // u8[] 紧凑布局同样安全裁剪
    const bytes = [1u8, 2u8, 3u8];
    const b_over = bytes[1:99];
    passed = passed & check(b_over.length == 2 && b_over[0] == 2 && b_over[1] == 3, "u8 slice clamp");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
