import { println } from "std/io";
import { combined_a } from "./lib-a";
import { combined_b, transform } from "./lib-b";

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
    passed = passed & check(combined_a() == "a+from-b", "a calls b across files");
    passed = passed & check(combined_b() == "b+from-a", "b calls a across files");
    passed = passed & check(transform(5) == 11, "cross-file function from circular module");
    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
