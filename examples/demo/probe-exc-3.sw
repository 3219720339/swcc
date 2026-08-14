import { println } from "std/io";

function boom(): void {
    throw "x";
}

function main(): int {
    try {
        boom();
    } catch (e: string) {
        println("caught");
    }
    return 1;
}
