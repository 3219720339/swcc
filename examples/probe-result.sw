import { println } from "std/io";
import { Result } from "std/result";

function parse(text: string): Result<int, string> {
    if (text == "42") {
        return Result.Ok(42);
    }
    return Result.Err("bad number");
}

function double_or_fail(text: string): Result<int, string> {
    const value = parse(text)?;
    return Result.Ok(value * 2);
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
    const doubled = double_or_fail("42");
    match (doubled) {
        Ok(value) => {
            passed = passed & check(value == 84, "try operator ok payload");
        }
        Err(message) => {
            println(`[FAIL] unexpected ${message}`);
        }
    }
    const failed = double_or_fail("x");
    match (failed) {
        Ok(_) => {
            println("[FAIL] expected err");
        }
        Err(message) => {
            passed = passed & check(message == "bad number", "try operator err propagation");
        }
    }
    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
