import { println } from "std/io";

function main(): int {
    println("before");
    try {
        println("in-try");
    } catch (e: string) {
        println("caught");
    }
    println("after");
    return 0;
}
