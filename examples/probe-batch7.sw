import { println } from "std/io";
import {
    replace_first,
    replace_last,
    split_once,
    count_words,
    pad_center,
    common_prefix,
    common_suffix,
    swap_case,
    zfill,
} from "std/string";
import {
    concat_int,
    concat_float,
    concat_string,
    insert_int,
    insert_float,
    insert_string,
    remove_at_int,
    remove_at_float,
    remove_at_string,
    unique_int,
    unique_float,
    min_index_float,
    max_index_float,
} from "std/array";
import {
    run_with_env,
    run_in_dir,
    run_stdout_stderr,
    is_process_running,
    spawn,
    wait,
    platform,
    mkdtemp,
} from "std/os";
import { map_new, map_set } from "std/map";
import {
    write_all,
    remove,
    mkdir,
    path_join,
    touch,
    is_writable,
    file_age_sec,
    dir_size,
} from "std/fs";
import {
    json_parse,
    json_int,
    json_object_get,
    json_object_len,
    json_has,
    json_get_path,
    json_merge,
    json_object_new,
    json_int_new,
    json_object_set,
    json_array_len,
} from "std/json";
import { temp_dir } from "std/os";

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

    // ---------- std/string 实用 ----------
    passed = passed & check(replace_first("aaa", "a", "X") == "Xaa", "replace_first");
    passed = passed & check(replace_first("abc", "z", "X") == "abc", "replace_first not found");
    passed = passed & check(replace_last("aaa", "a", "X") == "aaX", "replace_last");
    const once = split_once("a,b,c", ",");
    passed = passed & check(once.length == 2 && once[0] == "a" && once[1] == "b,c", "split_once");
    const once2 = split_once("abc", ",");
    passed = passed & check(once2.length == 2 && once2[0] == "abc" && once2[1] == "", "split_once not found");
    passed = passed & check(count_words("  hello   world ") == 2, "count_words");
    passed = passed & check(pad_center("ab", 6, "-") == "--ab--", "pad_center even");
    passed = passed & check(pad_center("a", 4, "-") == "-a--", "pad_center odd");
    passed = passed & check(common_prefix("hello123", "helloworld") == "hello", "common_prefix");
    passed = passed & check(common_prefix("你好abc", "你好xyz") == "你好", "common_prefix UTF-8");
    passed = passed & check(common_suffix("abc123", "xyz123") == "123", "common_suffix");
    passed = passed & check(swap_case("Hello123") == "hELLO123", "swap_case");
    passed = passed & check(zfill("42", 5) == "00042", "zfill");

    // ---------- std/array 实用 ----------
    const c1 = concat_int([1, 2], [3, 4]);
    passed = passed & check(c1.length == 4 && c1[0] == 1 && c1[3] == 4, "concat_int");
    const cs = concat_string(["a"], ["b", "c"]);
    passed = passed & check(cs.length == 3 && cs[2] == "c", "concat_string");
    const i1 = insert_int([1, 3], 1, 2);
    passed = passed & check(i1.length == 3 && i1[1] == 2, "insert_int");
    const i2 = insert_int([1, 2], 5, 9);
    passed = passed & check(i2.length == 3 && i2[2] == 9, "insert_int clamp");
    const i3 = insert_string(["a", "c"], 1, "b");
    passed = passed & check(i3.length == 3 && i3[1] == "b", "insert_string");
    const r1 = remove_at_int([1, 2, 3], 1);
    passed = passed & check(r1.length == 2 && r1[0] == 1 && r1[1] == 3, "remove_at_int");
    const r2 = remove_at_int([1, 2], 5);
    passed = passed & check(r2.length == 2 && r2[0] == 1, "remove_at_int out of range");
    const r3 = remove_at_string(["a", "b", "c"], 0);
    passed = passed & check(r3.length == 2 && r3[0] == "b", "remove_at_string");
    const u1 = unique_int([1, 2, 1, 3, 2]);
    passed = passed & check(u1.length == 3 && u1[0] == 1 && u1[2] == 3, "unique_int");
    const u2 = unique_float([1.5, 2.5, 1.5]);
    passed = passed & check(u2.length == 2 && u2[0] == 1.5, "unique_float");
    passed = passed & check(min_index_float([3.0, 1.5, 2.5]) == 1, "min_index_float");
    passed = passed & check(max_index_float([3.0, 1.5, 2.5]) == 0, "max_index_float");
    passed = passed & check(min_index_float([]) == -1, "min_index_float empty");

    // ---------- std/os 进程增强 ----------
    const win = platform() == "windows";
    // run_with_env：子进程读取环境变量
    const env = map_new();
    map_set(env, "SWTEST", "hello_env");
    const env_out = win
        ? run_with_env("cmd", ["/c", "echo %SWTEST%"], env)
        : run_with_env("sh", ["-c", "echo $SWTEST"], env);
    passed = passed & check(env_out.contains("hello_env"), "run_with_env passes env");

    // run_in_dir：子进程输出所在目录
    // 用唯一临时目录名断言（避免 macOS /var→/private/var 符号链接差异）。
    const work_dir = mkdtemp("swc-cwd-");
    const tmp = work_dir;
    const dir_out = win
        ? run_in_dir("cmd", ["/c", "cd"], tmp)
        : run_in_dir("pwd", [], tmp);
    passed = passed & check(dir_out.contains("swc-cwd-"), "run_in_dir cwd");

    // run_stdout_stderr：分开捕获
    const parts = win
        ? run_stdout_stderr("cmd", ["/c", "echo out && echo err 1>&2"])
        : run_stdout_stderr("sh", ["-c", "echo out; echo err >&2"]);
    passed = passed & check(parts.length == 2 && parts[0].contains("out"), "run_stdout_stderr stdout");
    passed = passed & check(parts[1].contains("err"), "run_stdout_stderr stderr");

    // is_process_running：spawn 后为 true，wait 后为 false
    const pid = win ? spawn("ping", ["-n", "5", "127.0.0.1"]) : spawn("sleep", ["5"]);
    passed = passed & check(pid != 0, "spawn ok");
    passed = passed & check(is_process_running(pid), "is_process_running true");
    wait(pid);
    passed = passed & check(!is_process_running(pid), "is_process_running false after wait");

    // ---------- std/fs 增强 ----------
    const dir = path_join(temp_dir(), "swcc-batch7");
    mkdir(dir);
    const f1 = path_join(dir, "a.txt");
    const f2 = path_join(dir, "b.txt");
    write_all(f1, "12345");
    write_all(f2, "678");
    passed = passed & check(dir_size(dir) == 8, "dir_size sums files");
    passed = passed & check(file_age_sec(f1) >= 0 && file_age_sec(f1) < 60, "file_age_sec fresh");
    passed = passed & check(file_age_sec(path_join(dir, "nope.txt")) == -1, "file_age_sec missing");
    passed = passed & check(is_writable(f1), "is_writable true");
    remove(f1);
    remove(f2);
    remove(dir);

    // ---------- std/json 读路径增强 ----------
    const doc = json_parse(`{"a": {"b": 42}, "c": "x", "arr": [1, 2]}`);
    passed = passed & check(json_int(json_get_path(doc, "a.b")) == 42, "json_get_path nested");
    passed = passed & check(json_get_path(doc, "a.x") == null, "json_get_path missing");
    passed = passed & check(json_object_len(json_get_path(doc, "a")) == 1, "json_object_len");
    passed = passed & check(json_has(doc, "c"), "json_has true");
    passed = passed & check(!json_has(doc, "z"), "json_has false");
    passed = passed & check(json_object_len(doc) == 3, "json_object_len top");
    passed = passed & check(json_array_len(json_object_get(doc, "arr")) == 2, "json_array_len");

    const ja = json_object_new();
    json_object_set(ja, "a", json_int_new(1));
    json_object_set(ja, "b", json_int_new(2));
    const jb = json_object_new();
    json_object_set(jb, "b", json_int_new(3));
    json_object_set(jb, "c", json_int_new(4));
    const merged = json_merge(ja, jb);
    passed = passed & check(json_object_len(merged) == 3, "json_merge len");
    passed = passed & check(json_int(json_object_get(merged, "b")) == 3, "json_merge override");
    passed = passed & check(json_int(json_object_get(merged, "c")) == 4, "json_merge add");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
