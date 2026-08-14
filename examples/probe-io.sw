import { println } from "std/io";
import { read_lines, write_all, append } from "std/fs";
import { is_number, parse_int_or, repeat } from "std/string";
import { date_string, now_sec } from "std/time";

function main(): int {
    write_all("io-sample.txt", "one\ntwo\nthree\n");
    append("io-sample.txt", "four\n");
    const lines = read_lines("io-sample.txt");
    const n = "42".parse_int_or(-1);
    const bad = "abc".parse_int_or(-1);
    const sep = "-".repeat(8);
    println(`${sep}`);
    println(`lines=${lines.length} first=${lines[0]} last=${lines[3]}`);
    println(`n=${n} bad=${bad} number=${is_number("3.14")} today=${date_string(now_sec())}`);
    return 0;
}
