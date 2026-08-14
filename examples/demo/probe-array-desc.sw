import { println } from "std/io";
import { sort_int, sort_int_desc, sort_float_desc, sort_string_desc } from "std/array";

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
    const a = [3, 1, 4, 1, 5];
    sort_int(a);
    passed = passed & check(a[0] == 1 && a[4] == 5, "sort_int asc still works");
    sort_int_desc(a);
    passed = passed & check(a[0] == 5 && a[4] == 1, "sort_int_desc descending");

    const f = [1.5, 0.5, 2.5];
    sort_float_desc(f);
    passed = passed & check(f[0] == 2.5 && f[2] == 0.5, "sort_float_desc");

    const s = ["pear", "apple", "banana"];
    sort_string_desc(s);
    passed = passed & check(s[0] == "pear" && s[2] == "apple", "sort_string_desc");

    // 空数组/单元素不崩。
    const empty: int[] = [];
    sort_int_desc(empty);
    passed = passed & check(empty.length == 0, "desc sort on empty array");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
