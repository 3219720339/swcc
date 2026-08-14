import { println } from "std/io";

function main(): int {
    const s = "  Hello, Sw  ";
    const cleaned = s.trim().to_upper();
    const joined = s.trim().split(" ").join("|");
    const n = "  42  ".trim().parse_int();
    const same = "abc" == "abc";
    const first = "你好Sw"[0];
    const len = "你好Sw".length;
    const replaced = "a-b-c".replace("-", "+");
    println(`cleaned=${cleaned} joined=${joined} n=${n} same=${same} first=${first} len=${len} replaced=${replaced}`);
    return 0;
}
