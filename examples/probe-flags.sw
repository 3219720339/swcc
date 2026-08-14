import { println } from "std/io";
import { flag_has, flag_value, 含参数, 取参数值 } from "std/os";

function check(condition: bool, label: string): int {
    if (condition) {
        println(`[ok] ${label}`);
        return 1;
    }
    println(`[FAIL] ${label}`);
    return 0;
}

function main(args: string[]): int {
    let passed = 1;

    // --name=value 形式
    passed = passed & check(flag_has(args, "--verbose"), "has verbose");
    passed = passed & check((flag_value(args, "--port") ?? "") == "8080", "value port");
    passed = passed & check((flag_value(args, "--host") ?? "") == "127.0.0.1", "value host");

    // --name value 形式
    passed = passed & check((flag_value(args, "--mode") ?? "") == "fast", "value mode space");

    // 短 flag
    passed = passed & check(flag_has(args, "-v"), "has short v");
    passed = passed & check(!flag_has(args, "--missing"), "missing flag");
    passed = passed & check(flag_value(args, "--missing") == null, "missing value null");

    // 中文名
    passed = passed & check(含参数(args, "--verbose"), "cn has");
    passed = passed & check((取参数值(args, "--port") ?? "") == "8080", "cn value");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
