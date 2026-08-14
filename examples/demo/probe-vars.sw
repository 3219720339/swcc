import { println } from "std/io";
import { VERSION, counter, bump } from "./lib-vars";

function check(prev: int, cond: bool, label: string): int {
    let state = "FAIL";
    if (cond) {
        state = "ok";
    }
    println(`[${state}] ${label}`);
    if (cond) {
        return prev;
    }
    return 0;
}

function main(): int {
    let ok = 1;
    ok = check(ok, VERSION == 3, "import_const");
    ok = check(ok, counter == 10, "import_global");
    ok = check(ok, bump() == 11, "bump_once");
    ok = check(ok, counter == 11, "global_after_bump");
    println(`final=${ok == 1 ? "PASS" : "FAIL"}`);
    return ok == 1 ? 0 : 1;
}
