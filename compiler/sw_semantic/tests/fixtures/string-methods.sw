function main(): int {
    const s = "  Hello, Sw  ";
    const a = s.trim().to_upper();
    const parts = s.trim().split(" ");
    const joined = parts.join("-");
    const n = "42".parse_int();
    const f = "3.5".parse_float() as int;
    const same = "abc" == "abc";
    const ch = "你好Sw"[0];
    const len = "你好Sw".length;
    const replaced = "a-b".replace("-", "+");
    if (a == "HELLO, SW" && joined == "Hello,-Sw" && n == 42 && f == 3 && same && ch == 20320 && len == 4 && replaced == "a+b") {
        return 0;
    }
    return 1;
}
