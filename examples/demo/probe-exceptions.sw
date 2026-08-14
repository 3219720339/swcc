import { println } from "std/io";

function risky(flag: bool): int {
    if (flag) {
        throw "boom";
    }
    return 42;
}

function main(): int {
    let result = 0;
    try {
        result = risky(true);
    } catch (e: string) {
        result = 1;
    }
    try {
        result = risky(false);
    } finally {
        println("finally-ok");
    }
    return result;
}
