import { println } from "std/io";
import {
    desktop_dir,
    documents_dir,
    downloads_dir,
    pictures_dir,
    music_dir,
    videos_dir,
    config_dir,
    system_dir,
    username,
    pid,
    arch,
    unsetenv,
    setenv,
    getenv,
    home_dir,
} from "std/os";
import {
    path_absolute,
    path_normalize,
    is_absolute,
    path_parts,
    expand_home,
    mkdir_p,
    disk_free,
    disk_total,
    is_symlink,
    read_symlink,
    file_mode,
    remove_all,
    is_dir,
    write_all,
} from "std/fs";
import { is_empty, utf8_is_valid, truncate, ellipsis } from "std/string";
import { deg_to_rad, rad_to_deg, is_nan, is_infinite, tau, pi, fabs } from "std/math";
import { parse_datetime, time_from_parts } from "std/time";
import { base64url_encode, base64url_decode, html_escape } from "std/encoding";
import {
    json_parse,
    json_stringify,
    json_object_keys,
    json_type_name,
    json_object_get,
    json_int,
    json_array_len,
} from "std/json";

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

    // 用户目录与系统信息
    ok = check(ok, desktop_dir().length > 0, "desktop_dir");
    ok = check(ok, documents_dir().length > 0, "documents_dir");
    ok = check(ok, downloads_dir().length > 0, "downloads_dir");
    ok = check(ok, pictures_dir().length > 0, "pictures_dir");
    ok = check(ok, music_dir().length > 0, "music_dir");
    ok = check(ok, videos_dir().length > 0, "videos_dir");
    ok = check(ok, config_dir().length > 0, "config_dir");
    ok = check(ok, system_dir().length > 0, "system_dir");
    ok = check(ok, username().length > 0, "username");
    ok = check(ok, pid() > 0, "pid");
    const arch_name = arch();
    ok = check(ok, arch_name == "x86_64" || arch_name == "aarch64", "arch");
    ok = check(ok, setenv("SW_TEST_VAR2", "xyz") == 0, "setenv2");
    ok = check(ok, (getenv("SW_TEST_VAR2") ?? "") == "xyz", "getenv2");
    ok = check(ok, unsetenv("SW_TEST_VAR2") == 0, "unsetenv");
    ok = check(ok, (getenv("SW_TEST_VAR2") ?? "") == "", "unsetenv_gone");

    // 路径工具
    const abs = path_absolute("x.txt");
    ok = check(ok, is_absolute(abs), "path_absolute");
    ok = check(ok, !is_absolute("x.txt"), "is_absolute_false");
    const norm = path_normalize("a/./b/../c");
    ok = check(ok, norm.index_of("..") < 0, "normalize_no_dotdot");
    ok = check(ok, norm.length > 0, "normalize_len");
    const parts = path_parts("a/b/c");
    ok = check(ok, parts.length == 3, "path_parts");
    const home = home_dir();
    ok = check(ok, expand_home("~").length > 0, "expand_home");
    ok = check(ok, is_absolute(expand_home("~")), "expand_home_abs");
    ok = check(ok, expand_home("~/x").index_of(home) == 0, "expand_home_prefix");

    // 递归建目录 / 磁盘 / 链接 / 权限
    ok = check(ok, mkdir_p("stdlib-mk/a/b/c") == 0, "mkdir_p");
    ok = check(ok, is_dir("stdlib-mk/a/b/c") == 1, "mkdir_p_dir");
    ok = check(ok, remove_all("stdlib-mk") == 0, "mkdir_p_cleanup");
    ok = check(ok, disk_free(".") > 0, "disk_free");
    ok = check(ok, disk_total(".") > 0, "disk_total");
    ok = check(ok, disk_total(".") >= disk_free("."), "disk_total_ge_free");
    ok = check(ok, write_all("stdlib-link-test.txt", "x") == 1, "link_write");
    ok = check(ok, !is_symlink("stdlib-link-test.txt"), "is_symlink_false");
    ok = check(ok, read_symlink("stdlib-link-test.txt").length == 0, "read_symlink_plain");
    ok = check(ok, file_mode("stdlib-link-test.txt") > 0, "file_mode");
    remove_all("stdlib-link-test.txt");

    // 字符串
    ok = check(ok, "".is_empty(), "is_empty");
    ok = check(ok, !"x".is_empty(), "is_empty_false");
    ok = check(ok, "中文".utf8_is_valid(), "utf8_valid");
    ok = check(ok, "abc".utf8_is_valid(), "utf8_valid_ascii");
    ok = check(ok, truncate("你好世界", 2) == "你好", "truncate");
    ok = check(ok, ellipsis("hello", 5) == "hello", "ellipsis_full");
    ok = check(ok, ellipsis("hello", 3) == "...", "ellipsis_short");
    ok = check(ok, ellipsis("hello", 4) == "h...", "ellipsis_4");

    // 数学
    ok = check(ok, fabs(deg_to_rad(180.0) - pi()) < 0.000000001, "deg_to_rad");
    ok = check(ok, fabs(rad_to_deg(pi()) - 180.0) < 0.000000001, "rad_to_deg");
    ok = check(ok, is_nan(0.0 / 0.0), "is_nan");
    ok = check(ok, !is_nan(1.0), "is_nan_false");
    ok = check(ok, is_infinite(1.0 / 0.0), "is_infinite");
    ok = check(ok, !is_infinite(1.0), "is_infinite_false");
    ok = check(ok, fabs(tau() - 2.0 * pi()) < 0.000000001, "tau");

    // 时间
    ok = check(
        ok,
        parse_datetime("2026-01-02 03:04:05") == time_from_parts(2026, 1, 2, 3, 4, 5),
        "parse_datetime"
    );
    ok = check(
        ok,
        parse_datetime("2026-01-02T03:04:05") == time_from_parts(2026, 1, 2, 3, 4, 5),
        "parse_datetime_t"
    );
    ok = check(ok, parse_datetime("bad-date") == -1, "parse_datetime_bad");

    // 编码
    ok = check(ok, base64url_encode("hello") == "aGVsbG8", "base64url_encode");
    ok = check(ok, base64url_decode("aGVsbG8") == "hello", "base64url_decode");
    ok = check(ok, html_escape("<a&\"b'>") == "&lt;a&amp;&quot;b&#39;&gt;", "html_escape");

    // JSON 序列化与键
    const doc = json_parse("{\"a\":1,\"b\":[true,null,\"x\"]}");
    ok = check(ok, json_type_name(doc) == "object", "json_type_object");
    const keys = json_object_keys(doc);
    ok = check(ok, keys.length == 2, "json_keys_len");
    ok = check(ok, json_int(json_object_get(doc, "a")) == 1, "json_get_a");
    const arr = json_object_get(doc, "b");
    ok = check(ok, json_type_name(arr) == "array", "json_type_array");
    ok = check(ok, json_array_len(arr) == 3, "json_array_len");
    const text = json_stringify(doc);
    ok = check(ok, text.index_of("\"a\":1") >= 0, "json_stringify_a");
    ok = check(ok, text.index_of("true") >= 0, "json_stringify_true");
    ok = check(ok, text.index_of("null") >= 0, "json_stringify_null");
    const doc2 = json_parse(text);
    ok = check(ok, json_int(json_object_get(doc2, "a")) == 1, "json_roundtrip");
    ok = check(ok, json_stringify(json_parse("null")) == "null", "json_null_str");
    ok = check(ok, json_stringify(json_parse("1.5")) == "1.5", "json_float_str");
    ok = check(ok, json_stringify(json_parse("true")) == "true", "json_bool_str");
    ok = check(ok, json_stringify(json_parse("\"hi\"")) == "\"hi\"", "json_string_str");

    println(`final=${ok == 1 ? "PASS" : "FAIL"}`);
    return ok == 1 ? 0 : 1;
}
