import { println } from "std/io";
import { write_all, chmod, remove, exists, touch, file_size_path } from "std/fs";

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
    ok = check(ok, write_all("chmod-test.txt", "hello") == 5, "write");
    ok = check(ok, file_size_path("chmod-test.txt") == 5, "size");
    // 0o644 = 420；0o400 = 256（只读）；0o777 = 511（恢复可写）。
    ok = check(ok, chmod("chmod-test.txt", 420) == 0, "chmod_644");
    ok = check(ok, chmod("chmod-test.txt", 256) == 0, "chmod_400");
    ok = check(ok, chmod("chmod-test.txt", 511) == 0, "chmod_777");
    ok = check(ok, touch("chmod-test.txt") == 0, "touch");
    ok = check(ok, remove("chmod-test.txt") == 0, "remove");
    ok = check(ok, exists("chmod-test.txt") == 0, "removed");
    println(`final=${ok == 1 ? "PASS" : "FAIL"}`);
    return ok == 1 ? 0 : 1;
}
