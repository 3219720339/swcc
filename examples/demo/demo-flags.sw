import { println } from "std/io";
import { flag_has, flag_value, 含参数, 取参数值 } from "std/os";

function main(args: string[]): int {
    println(flag_has(args, "--verbose"));
    println(flag_has(args, "-v"));
    println(flag_has(args, "--missing"));
    println((flag_value(args, "--port") ?? "(无)"));
    println((flag_value(args, "--host") ?? "(无)"));
    println((flag_value(args, "--mode") ?? "(无)"));
    println((flag_value(args, "--missing") ?? "(无)"));
    println(含参数(args, "--verbose"));
    println((取参数值(args, "--port") ?? "(无)"));
    return 0;
}
