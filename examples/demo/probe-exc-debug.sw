import { println } from "std/io";

function main(): int {
    println("before-try");
    try {
        println("in-try");
        throw "x";
        println("after-throw");
    } catch (e: string) {
        println("caught");
        return 1;
    }
    return 0;
}
