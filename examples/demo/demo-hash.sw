import { println } from "std/io";
import { fnv1a_64, fnv1a_64_seed, djb2 } from "std/hash";

function main(): int {
    println(`fnv1a_64("hello")=${fnv1a_64("hello")}`);
    println(`fnv1a_64("sw")=${fnv1a_64("sw")}`);
    println(`fnv1a_64_seed("hello", 0)=${fnv1a_64_seed("hello", 0)}`);
    println(`fnv1a_64_seed("hello", 42)=${fnv1a_64_seed("hello", 42)}`);
    println(`djb2("hello")=${djb2("hello")}`);
    println(`djb2("sw")=${djb2("sw")}`);
    return 0;
}
