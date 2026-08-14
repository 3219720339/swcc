// probe-cross-stdlib.sw：主文件调用 lib-std.sw，
// 被调用模块内部全部使用基础标准库。
import { println } from "std/io";
import {
    describe,
    math_report,
    today,
    now_text,
    array_sum,
    unique_words,
    counter_report,
    json_report,
    hash_report,
    regex_report,
    file_report,
    encode_report,
    random_report,
} from "./lib-std";

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

    passed = passed & check(describe("sw") == "SW-ws", "describe string stdlib");
    passed = passed & check(math_report() == "gcd=6 sqrt=4 pi=3.14", "math stdlib");
    const d = today();
    passed = passed & check(d.length == 10, "date stdlib");
    const t = now_text();
    passed = passed & check(t.length == 19, "datetime stdlib");

    const nums = [3, 1, 2];
    passed = passed & check(array_sum(nums) == 6, "array stdlib");
    passed = passed & check(unique_words(["a", "b", "a"]) == "a,b", "unique stdlib");
    passed = passed & check(counter_report() == "name=sw count=5", "map stdlib");
    passed = passed & check(json_report() == "lang=sw year=2026", "json stdlib");
    passed = passed & check(hash_report() == "md5=5d41402abc4b2a76b9719d911017c592 sha256_len=64", "hash stdlib");
    passed = passed & check(regex_report("a1 b22 c333") == "first=1 all=3", "regex stdlib");
    passed = passed & check(file_report(".") == "exists=1 lines=3 first=one", "fs stdlib");
    passed = passed & check(encode_report() == "b64=aGk= hex=6869", "encoding stdlib");
    const r = random_report();
    passed = passed & check(r.starts_with("rand="), "random stdlib");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
