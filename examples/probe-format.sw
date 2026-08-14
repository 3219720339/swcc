import { println } from "std/io";
import {
    reverse,
    pad_left,
    pad_right,
    format_int,
    format_float,
} from "std/string";
import { rand_int, clamp, gcd, lcm } from "std/math";
import { datetime_string, parse_date } from "std/time";

function main(): int {
    const rev = reverse("你好Sw");
    const p1 = pad_left("42", 5, "0");
    const p2 = pad_right("42", 5, "-");
    const f1 = format_int(42, 3, 1);
    const f2 = format_float(3.14159, 2);
    const c = clamp(50, 0, 10);
    const g = gcd(12, 18);
    const l = lcm(4, 6);
    const r = rand_int(100);
    const dt = datetime_string(parse_date("2026-01-02"));
    println(`rev=${rev} p1=${p1} p2=${p2} f1=${f1} f2=${f2}`);
    println(`clamp=${c} gcd=${g} lcm=${l} rand=${r} dt=${dt}`);
    return 0;
}
