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

function check(condition: bool, label: string): int {
    if (condition) {
        println(`[ok] ${label}`);
        return 1;
    }
    println(`[FAIL] ${label}`);
    return 0;
}

function main(): int {
    let passed = 1;

    // MD5 已知向量：空串 与 "abc"
    passed = passed & check(md5("") == "d41d8cd98f00b204e9800998ecf8427e", "md5 empty");
    passed = passed & check(md5("abc") == "900150983cd24fb0d6963f7d28e17f72", "md5 abc");
    passed = passed & check(md5("hello") == "5d41402abc4b2a76b9719d911017c592", "md5 hello");
    passed = passed & check(md5("你好") == "7eca689f0d3389d9dea66ae112e5cfd7", "md5 chinese");

    // SHA-256 已知向量
    passed = passed & check(sha256("") == "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855", "sha256 empty");
    passed = passed & check(sha256("abc") == "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad", "sha256 abc");
    passed = passed & check(sha256("hello") == "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824", "sha256 hello");

    // 中文名
    passed = passed & check(取MD5("abc") == "900150983cd24fb0d6963f7d28e17f72", "cn md5");
    passed = passed & check(取SHA256("abc") == "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad", "cn sha256");

    // 文件版
    write_all("hash-tmp.txt", "abc");
    passed = passed & check(md5_file("hash-tmp.txt") == "900150983cd24fb0d6963f7d28e17f72", "md5 file");
    passed = passed & check(sha256_file("hash-tmp.txt") == "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad", "sha256 file");
    passed = passed & check(取MD5文件("hash-tmp.txt") == "900150983cd24fb0d6963f7d28e17f72", "cn md5 file");
    passed = passed & check(取SHA256文件("hash-tmp.txt") == "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad", "cn sha256 file");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
