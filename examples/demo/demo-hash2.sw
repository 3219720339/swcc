import { println } from "std/io";
import {
    md5,
    md5_file,
    sha256,
    sha256_file,
    取MD5,
    取SHA256,
    取MD5文件,
    取SHA256文件,
} from "std/hash";
import { write_all } from "std/fs";

function main(): int {
    println(md5("hello"));
    println(md5("你好"));
    println(sha256("hello"));
    println(sha256("abc"));
    println(取MD5("abc"));
    println(取SHA256("abc"));
    write_all("hash-demo.txt", "abc");
    println(md5_file("hash-demo.txt"));
    println(sha256_file("hash-demo.txt"));
    println(取MD5文件("hash-demo.txt"));
    println(取SHA256文件("hash-demo.txt"));
    return 0;
}
