import { println } from "std/io";

class Utils {
    static version: int;
    static enabled: bool;

    static make_label(prefix: string, value: int): string {
        return prefix + value;
    }

    static triple(x: int): int {
        return x * 3;
    }
}

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
    passed = passed & check(Utils.triple(5) == 15, "static method call");
    passed = passed & check(Utils.make_label("v", 7) == "v7", "static method with args");
    passed = passed & check(Utils.version == 0, "static field default");
    passed = passed & check(Utils.enabled == false, "static bool field default");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
