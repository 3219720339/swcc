import { println, print_format } from "std/io";
import { replace_first, common_prefix, zfill, swap_case, split_once, format } from "std/string";
import { concat_int, insert_int, remove_at_int, unique_int } from "std/array";
import { run_with_env, run_stdout_stderr, platform } from "std/os";
import { map_new, map_set } from "std/map";
import {
    json_parse,
    json_int,
    json_string,
    json_get_path,
    json_merge,
    json_object_new,
    json_int_new,
    json_object_set,
    json_object_len,
} from "std/json";

// 标准库实用批演示：string/array 组合、进程增强、json 读路径。
function main(): int {
    println("== string 实用 ==");
    println(replace_first("banana", "na", "X"));          // baXna
    println(zfill("42", 5));                              // 00042
    println(swap_case("Hello World"));                    // hELLO wORLD
    println(common_prefix("swcc-1.0", "swcc-2.0"));       // swcc-
    const once = split_once("host:8080/path", ":");
    print_format("left=%s right=%s", once[0], once[1]);   // left=host right=8080/path
    println("");

    println("== array 实用 ==");
    const nums = insert_int([1, 3], 1, 2);                // [1,2,3]
    print_format("insert=%s", join_int(nums));            // 1,2,3
    println("");
    const merged = concat_int([1, 2], [3]);
    print_format("concat=%s dedupe=%s", join_int(merged), join_int(unique_int([1, 1, 2])));
    println("");

    println("== 进程增强 ==");
    const env = map_new();
    map_set(env, "SW_HELLO", "from-env");
    const env_out = platform() == "windows"
        ? run_with_env("cmd", ["/c", "echo %SW_HELLO%"], env)
        : run_with_env("sh", ["-c", "echo $SW_HELLO"], env);
    print_format("run_with_env=%s", env_out.trim());
    println("");
    const parts = platform() == "windows"
        ? run_stdout_stderr("cmd", ["/c", "echo to-stdout && echo to-stderr 1>&2"])
        : run_stdout_stderr("sh", ["-c", "echo to-stdout; echo to-stderr >&2"]);
    print_format("stdout=[%s] stderr=[%s]", parts[0].trim(), parts[1].trim());
    println("");

    println("== json 读路径 ==");
    const doc = json_parse(`{"server": {"host": "localhost", "port": 8080}}`);
    print_format("host=%s port=%d", json_string(json_get_path(doc, "server.host")), json_int(json_get_path(doc, "server.port")));
    println("");
    const a = json_object_new();
    json_object_set(a, "x", json_int_new(1));
    const b = json_object_new();
    json_object_set(b, "y", json_int_new(2));
    print_format("merged len=%d", json_object_len(json_merge(a, b)));
    println("");
    return 0;
}

function join_int(items: int[]): string {
    let result = "";
    for (const item of items) {
        if (result != "") {
            result = result + ",";
        }
        result = result + format("%d", item);
    }
    return result;
}
