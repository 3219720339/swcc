// 符号链接策略负例：walk_files/walk_dirs 不跟随链接、copy_dir 遇链接失败、
// remove_all 只删链接本身不进入目标。若当前环境无法创建符号链接（如 Windows
// 未开开发者模式/无权限），打印 [skip] 并以 0 退出，避免 CI 无权限失败。
import { println } from "std/io";
import { run, platform } from "std/os";
import {
    mkdir,
    mkdir_p,
    write_all,
    walk_files,
    copy_dir,
    remove_all,
    is_symlink,
    exists,
    path_absolute,
} from "std/fs";

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

    // 1) 准备：真实目录 + 文件，再创建指向它的符号链接。
    remove_all("symlink-probe");
    mkdir_p("symlink-probe/real");
    write_all("symlink-probe/real/data.txt", "secret");
    if (platform() == "windows") {
        const abs = path_absolute("symlink-probe/real");
        run("cmd", ["/c", "mklink", "/d", "symlink-probe\\real-link", abs]);
    } else {
        run("ln", ["-s", "real", "symlink-probe/real-link"]);
    }
    if (!is_symlink("symlink-probe/real-link")) {
        println("[skip] 无法创建符号链接（无权限）");
        remove_all("symlink-probe");
        return 0;
    }
    passed = passed & check(is_symlink("symlink-probe/real-link"), "is_symlink true");

    // 2) walk_files 不跟随链接：不应出现 real-link/data.txt（不会递归进入链接）。
    const files = walk_files("symlink-probe");
    let has_link_content = false;
    let i = 0;
    while (i < files.length) {
        if (files[i].index_of("real-link") >= 0 && files[i].index_of("data.txt") >= 0) {
            has_link_content = true;
        }
        i = i + 1;
    }
    passed = passed & check(!has_link_content, "walk_files not follow symlink");

    // 3) copy_dir 遇链接失败（返回 -1）。
    const copied = copy_dir("symlink-probe", "symlink-probe-copy");
    passed = passed & check(copied != 0, "copy_dir fails on symlink");

    // 4) remove_all 只删链接本身：链接消失但目标目录与文件仍在。
    remove_all("symlink-probe/real-link");
    passed = passed & check(exists("symlink-probe/real-link") == 0, "remove_all link gone");
    passed = passed & check(exists("symlink-probe/real/data.txt") == 1, "remove_all target intact");

    // 5) 清理。
    remove_all("symlink-probe");
    remove_all("symlink-probe-copy");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
