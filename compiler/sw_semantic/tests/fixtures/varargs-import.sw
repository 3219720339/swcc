import * from "./lib/calc";
import "./lib/calc";
import { format } from "std/string";
import { walk_files, read_file_bytes, write_file_bytes } from "std/fs";

function main(): int {
    const a = add(1, 2);
    const b = mul(3, 4);
    const c = calc.add(5, 6);
    const f1 = format("%s %d %.1f", "x", 7, 2.5);
    const f2 = format("hex=%x", 255);
    const f3 = format("pad=%05d", 9);
    const bytes: u8[] = [65u8, 66u8];
    const files = walk_files(".");
    const read = read_file_bytes("a.dat");
    write_file_bytes("a.dat", bytes);
    if (a == 3 && b == 12 && c == 11 && f1 == "x 7 2.5" && f2 == "hex=ff" && f3 == "pad=00009" && read.length >= 0 && files.length >= 0) {
        return 0;
    }
    return 1;
}
