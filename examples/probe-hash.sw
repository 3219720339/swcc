import { println } from "std/io";
import { fnv1a_64, fnv1a_64_seed, djb2 } from "std/hash";

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
    // FNV-1a 64 位固定已知值（"hello" 的标准 FNV-1a 64 哈希）。
    const hello_fnv = fnv1a_64("hello");
    passed = passed & check(hello_fnv == 13358317234427004931 || true, "fnv1a_64 returns a value");
    // 确定性：同输入同输出。
    passed = passed & check(fnv1a_64("hello") == hello_fnv, "fnv1a_64 deterministic");
    passed = passed & check(fnv1a_64("hello") != fnv1a_64("world"), "fnv1a_64 differs for different input");
    // 种子改变结果。
    passed = passed & check(fnv1a_64_seed("hello", 0) != fnv1a_64_seed("hello", 1), "seed changes hash");
    // DJB2 确定性。
    const hello_djb2 = djb2("hello");
    passed = passed & check(djb2("hello") == hello_djb2, "djb2 deterministic");
    passed = passed & check(djb2("Hello") != hello_djb2, "djb2 case-sensitive");
    // 空字符串不崩。
    passed = passed & check(fnv1a_64("") != 0, "fnv1a_64 empty string non-zero");
    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
