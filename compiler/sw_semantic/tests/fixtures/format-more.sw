import {
    reverse,
    pad_left,
    format_int,
    format_float,
    index_of_char,
} from "std/string";
import { clamp, gcd, lcm } from "std/math";
import { datetime_string, parse_date } from "std/time";

function main(): int {
    const rev = reverse("abc");
    const p1 = pad_left("42", 3, "0");
    const f1 = format_int(7, 3, 1);
    const f2 = format_float(2.5, 1);
    const idx = index_of_char("你好a", "你");
    const c = clamp(9, 0, 5);
    const g = gcd(12, 18);
    const l = lcm(4, 6);
    const dt = datetime_string(parse_date("2026-01-02"));
    if (
        rev == "cba" &&
        p1 == "042" &&
        f1 == "007" &&
        f2 == "2.5" &&
        idx == 0 &&
        c == 5 &&
        g == 6 &&
        l == 12 &&
        dt == "2026-01-02 00:00:00"
    ) {
        return 0;
    }
    return 1;
}
