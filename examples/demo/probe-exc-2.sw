import { println } from "std/io";

function main(): int {
    try {
        throw "x";
    } catch (e: string) {
    }
    println("after-try");
    return 1;
}
