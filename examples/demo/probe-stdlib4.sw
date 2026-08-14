import { println } from "std/io";
import {
    contains_int,
    contains_float,
    contains_string,
    index_of_int,
    index_of_float,
    index_of_string,
} from "std/array";

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
    const path = "C:/data/report.txt";
    passed = passed & check(path.remove_prefix("C:/") == "data/report.txt", "remove_prefix");
    passed = passed & check(path.remove_suffix(".txt") == "C:/data/report", "remove_suffix");
    passed = passed & check("report".remove_prefix("data") == "report", "remove_prefix no match");
    passed = passed & check("HELLO".is_upper(), "is_upper");
    passed = passed & check("hello".is_lower(), "is_lower");
    passed = passed & check("Hello".is_upper() == false, "is_upper mixed false");
    passed = passed & check("12345".is_digit(), "is_digit");
    passed = passed & check("12a45".is_digit() == false, "is_digit false");
    passed = passed & check("hello world".capitalize() == "Hello world", "capitalize");

    const nums = [10, 20, 30];
    const floats = [1.5, 2.5, 3.5];
    const words = ["apple", "banana", "cherry"];
    passed = passed & check(contains_int(nums, 20), "contains_int true");
    passed = passed & check(contains_int(nums, 99) == false, "contains_int false");
    passed = passed & check(contains_float(floats, 2.5), "contains_float true");
    passed = passed & check(contains_string(words, "banana"), "contains_string true");
    passed = passed & check(index_of_int(nums, 30) == 2, "index_of_int");
    passed = passed & check(index_of_int(nums, 99) == -1, "index_of_int missing");
    passed = passed & check(index_of_float(floats, 1.5) == 0, "index_of_float");
    passed = passed & check(index_of_string(words, "cherry") == 2, "index_of_string");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
