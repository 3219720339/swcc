import { println } from "std/io";
import { parse_bool } from "std/string";

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
    passed = passed & check(parse_bool("true") == true, "parse_bool true");
    passed = passed & check(parse_bool("TRUE") == true, "parse_bool TRUE upper");
    passed = passed & check(parse_bool("1") == true, "parse_bool 1");
    passed = passed & check(parse_bool("yes") == true, "parse_bool yes");
    passed = passed & check(parse_bool("false") == false, "parse_bool false");
    passed = passed & check(parse_bool("0") == false, "parse_bool 0");
    passed = passed & check(parse_bool("no") == false, "parse_bool no");
    passed = passed & check(parse_bool("garbage") == false, "parse_bool invalid -> false");
    passed = passed & check("on".parse_bool() == false, "parse_bool chain method");
    passed = passed & check("YES".parse_bool() == true, "parse_bool chain method YES");
    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
