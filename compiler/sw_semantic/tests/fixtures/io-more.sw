import { read_lines, write_all, append } from "std/fs";
import { is_number, parse_int_or, parse_float_or, repeat, from_code_point } from "std/string";
import { now_sec, date_string } from "std/time";

function main(): int {
    const n = "42".parse_int_or(-1);
    const bad = "abc".parse_int_or(-1);
    const f = "3.5".parse_float_or(0.0) as int;
    const ok = is_number("3.14");
    const not_ok = is_number("12x");
    const rep = "ab".repeat(2);
    const cp = from_code_point(65);
    const today = date_string(now_sec());
    if (
        n == 42 &&
        bad == -1 &&
        f == 3 &&
        ok &&
        !not_ok &&
        rep == "abab" &&
        cp == "A" &&
        today.length == 10
    ) {
        return 0;
    }
    return 1;
}
