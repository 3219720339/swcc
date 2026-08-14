import { println } from "std/io";

function fib(n: int): int {
    if (n < 2) {
        return n;
    }
    return fib(n - 1) + fib(n - 2);
}

function main(): int {
    println(`fib(10) = ${fib(10)}`);
    return 0;
}
