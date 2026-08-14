import { println } from "std/io";
import { getenv } from "std/os";

function main(args: string[]): int {
    const name = args.length > 1 ? args[1] : "world";
    println(`hello ${name}`);
    const home = getenv("USERPROFILE");
    println(`home=${home ?? "(none)"}`);
    return 0;
}
