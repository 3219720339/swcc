import { println, eprintln } from "std/io";
import {
    base64_encode,
    base64_decode,
    hex_encode,
    hex_decode,
    url_encode,
    url_decode,
} from "std/encoding";
import {
    ends_with,
    trim_left,
    trim_right,
    lines,
    split_whitespace,
    count,
    last_index_of,
    chars,
    from_utf8_bytes,
    to_utf8_bytes,
    is_ascii,
    escape,
    unescape,
} from "std/string";
import {
    round,
    trunc,
    sign,
    sin,
    cos,
    tan,
    asin,
    acos,
    atan,
    atan2,
    exp,
    log,
    log2,
    log10,
    hypot,
    cbrt,
    fmin,
    fmax,
    fabs,
    rand_float,
    rand_range,
    pi,
    e,
} from "std/math";
import { time_format, time_from_parts, timezone_offset_sec, now_sec } from "std/time";
import {
    cwd,
    chdir,
    temp_dir,
    home_dir,
    hostname,
    cpu_count,
    env_keys,
    setenv,
    getenv,
} from "std/os";
import {
    file_size_path,
    file_mtime,
    is_file,
    chmod,
    touch,
    copy_dir,
    remove_all,
    glob,
    exists,
    is_dir,
    mkdir,
    write_all,
    remove,
} from "std/fs";

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

    // 编码
    ok = check(ok, base64_encode("hello") == "aGVsbG8=", "base64_encode");
    ok = check(ok, base64_decode("aGVsbG8=") == "hello", "base64_decode");
    ok = check(ok, hex_encode("hello") == "68656c6c6f", "hex_encode");
    ok = check(ok, hex_decode("68656C6C6F") == "hello", "hex_decode");
    ok = check(ok, url_encode("a b&c") == "a%20b%26c", "url_encode");
    ok = check(ok, url_decode("a%20b%26c") == "a b&c", "url_decode");
    ok = check(ok, url_decode(url_encode("中 文")) == "中 文", "url_roundtrip");

    // 字符串补充（链式方法）
    ok = check(ok, "hello".ends_with("lo"), "ends_with");
    ok = check(ok, !"hello".ends_with("x"), "ends_with_false");
    ok = check(ok, "  x  ".trim_left() == "x  ", "trim_left");
    ok = check(ok, "  x  ".trim_right() == "  x", "trim_right");
    const ls = "a\nb\r\nc\n".lines();
    ok = check(ok, ls.length == 3, "lines.len");
    ok = check(ok, ls[1] == "b", "lines[1]");
    const ws = "a b  c\t d".split_whitespace();
    ok = check(ok, ws.length == 4, "split_whitespace");
    ok = check(ok, "banana".count("an") == 2, "count");
    ok = check(ok, "hello".last_index_of("l") == 3, "last_index_of");
    const cs = "中文a".chars();
    ok = check(ok, cs.length == 3, "chars.len");
    ok = check(ok, cs[0] == "中", "chars[0]");
    const bytes = to_utf8_bytes("ab");
    ok = check(ok, from_utf8_bytes(bytes) == "ab", "utf8_bytes");
    ok = check(ok, "abc".is_ascii(), "is_ascii");
    ok = check(ok, !"中文".is_ascii(), "is_ascii_cn");
    ok = check(ok, unescape(escape("a\"b\n\\c")) == "a\"b\n\\c", "escape_roundtrip");

    // 数学
    ok = check(ok, round(2.5) == 3.0, "round");
    ok = check(ok, round(-2.5) == -3.0, "round_neg");
    ok = check(ok, trunc(2.7) == 2.0, "trunc");
    ok = check(ok, sign(-5.0) == -1.0, "sign_neg");
    ok = check(ok, sign(0.0) == 0.0, "sign_zero");
    ok = check(ok, sign(7.0) == 1.0, "sign_pos");
    ok = check(ok, sin(0.0) == 0.0, "sin0");
    ok = check(ok, cos(0.0) == 1.0, "cos0");
    ok = check(ok, tan(0.0) == 0.0, "tan0");
    ok = check(ok, asin(0.0) == 0.0, "asin0");
    ok = check(ok, acos(1.0) == 0.0, "acos1");
    ok = check(ok, atan(0.0) == 0.0, "atan0");
    ok = check(ok, atan2(0.0, 1.0) == 0.0, "atan2");
    ok = check(ok, exp(0.0) == 1.0, "exp0");
    ok = check(ok, log(1.0) == 0.0, "log1");
    ok = check(ok, fabs(log2(2.0) - 1.0) < 0.000000001, "log2");
    ok = check(ok, fabs(log10(10.0) - 1.0) < 0.000000001, "log10");
    ok = check(ok, fabs(hypot(3.0, 4.0) - 5.0) < 0.000000001, "hypot");
    ok = check(ok, fabs(cbrt(8.0) - 2.0) < 0.000000001, "cbrt");
    ok = check(ok, fmin(1.0, 2.0) == 1.0, "fmin");
    ok = check(ok, fmax(1.0, 2.0) == 2.0, "fmax");
    ok = check(ok, pi() > 3.14, "pi_lo");
    ok = check(ok, pi() < 3.15, "pi_hi");
    ok = check(ok, e() > 2.71, "e");
    const rf = rand_float();
    ok = check(ok, rf >= 0.0, "rand_float_lo");
    ok = check(ok, rf < 1.0, "rand_float_hi");
    const rr = rand_range(5.0, 10.0);
    ok = check(ok, rr >= 5.0, "rand_range_lo");
    ok = check(ok, rr < 10.0, "rand_range_hi");

    // 时间
    const tp = time_from_parts(2026, 1, 2, 3, 4, 5);
    ok = check(
        ok,
        time_format(tp, "%Y-%m-%d %H:%M:%S") == "2026-01-02 03:04:05",
        "time_parts_format"
    );
    const tz = timezone_offset_sec();
    ok = check(ok, tz >= -14 * 3600, "tz_lo");
    ok = check(ok, tz <= 14 * 3600, "tz_hi");
    ok = check(ok, time_format(now_sec(), "%Y").length == 4, "year_len");

    // 系统信息
    const cur = cwd();
    ok = check(ok, cur.length > 0, "cwd");
    ok = check(ok, chdir(cur) == 0, "chdir");
    ok = check(ok, temp_dir().length > 0, "temp_dir");
    ok = check(ok, home_dir().length > 0, "home_dir");
    ok = check(ok, hostname().length > 0, "hostname");
    ok = check(ok, cpu_count() >= 1, "cpu_count");
    ok = check(ok, env_keys().length > 0, "env_keys");
    ok = check(ok, setenv("SW_TEST_VAR", "abc") == 0, "setenv");
    const ev = getenv("SW_TEST_VAR") ?? "";
    ok = check(ok, ev == "abc", "getenv_after_setenv");

    // 文件系统
    ok = check(ok, write_all("stdlib-test.txt", "12345") == 5, "write_all");
    ok = check(ok, file_size_path("stdlib-test.txt") == 5, "file_size_path");
    ok = check(ok, file_mtime("stdlib-test.txt") > 0, "file_mtime");
    ok = check(ok, is_file("stdlib-test.txt") == 1, "is_file");
    ok = check(ok, is_file(".") == 0, "is_file_dir");
    ok = check(ok, chmod("stdlib-test.txt", 420) == 0, "chmod");
    ok = check(ok, touch("stdlib-touch.txt") == 0, "touch");
    ok = check(ok, exists("stdlib-touch.txt") == 1, "touch_exists");
    ok = check(ok, mkdir("stdlib-dir-src") == 0, "mkdir_src");
    ok = check(ok, write_all("stdlib-dir-src/a.txt", "x") == 1, "write_src");
    ok = check(ok, copy_dir("stdlib-dir-src", "stdlib-dir-dst") == 0, "copy_dir");
    ok = check(ok, is_dir("stdlib-dir-dst") == 1, "copy_dir_is_dir");
    ok = check(ok, exists("stdlib-dir-dst/a.txt") == 1, "copy_dir_file");
    ok = check(ok, mkdir("stdlib-dir-rm") == 0, "mkdir_rm");
    ok = check(ok, mkdir("stdlib-dir-rm/sub") == 0, "mkdir_rm_sub");
    ok = check(ok, write_all("stdlib-dir-rm/sub/f.txt", "x") == 1, "write_rm");
    ok = check(ok, remove_all("stdlib-dir-rm") == 0, "remove_all");
    ok = check(ok, exists("stdlib-dir-rm") == 0, "remove_all_gone");
    ok = check(ok, mkdir("stdlib-glob") == 0, "mkdir_glob");
    ok = check(ok, write_all("stdlib-glob/a1.txt", "x") == 1, "write_g1");
    ok = check(ok, write_all("stdlib-glob/a2.txt", "x") == 1, "write_g2");
    ok = check(ok, write_all("stdlib-glob/b.dat", "x") == 1, "write_g3");
    const gs = glob("stdlib-glob/*.txt");
    const gj = gs.join(",");
    ok = check(ok, gs.length == 2, "glob_len");
    ok = check(ok, gj.index_of("a1.txt") >= 0, "glob_a1");
    ok = check(ok, gj.index_of("a2.txt") >= 0, "glob_a2");
    ok = check(ok, gj.index_of("b.dat") < 0, "glob_no_b");

    // 清理本次测试文件（仅删除自己创建的）
    chmod("stdlib-test.txt", 511);  // 恢复可写（0o777），Windows chmod 测试后文件是只读
    remove("stdlib-test.txt");
    remove("stdlib-touch.txt");
    remove_all("stdlib-dir-src");
    remove_all("stdlib-dir-dst");
    remove_all("stdlib-glob");

    eprintln("probe-stdlib stderr ok");
    println(`final=${ok == 1 ? "PASS" : "FAIL"}`);
    return ok == 1 ? 0 : 1;
}
