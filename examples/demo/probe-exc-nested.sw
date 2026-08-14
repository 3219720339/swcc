import { println } from "std/io";

function inner(): void {
    throw "deep";
}

function middle(): void {
    try {
        inner();
    } catch (e: int) {
        println("wrong-type");
    }
}

function main(): int {
    try {
        middle();
    } catch (e: string) {
        println("outer-caught");
        return 1;
    }
    return 0;
}
