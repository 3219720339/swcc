import { println } from "std/io";
import {
    platform,
    process_open,
    process_write,
    process_read_line,
    process_read_some,
    process_poll,
    process_wait,
    process_close_input,
} from "std/os";
import { sleep_ms } from "std/time";
import { contains, index_of } from "std/string";
import { progress_text, console_progress } from "std/console";

function check(condition: bool, label: string): int {
    if (condition) {
        println(`[ok] ${label}`);
        return 1;
    }
    println(`[FAIL] ${label}`);
    return 0;
}

// 非阻塞等子进程输出：poll==1 才读（避免阻塞），EOF（poll==-1）返回 ""。
function read_line_wait(proc: int): string {
    let guard = 0;
    while (guard < 200) {
        const status = process_poll(proc);
        if (status == 1) {
            return process_read_line(proc);
        }
        if (status == -1) {
            return "";
        }
        sleep_ms(10);
        guard++;
    }
    return "";
}

function main(): int {
    let passed = 1;
    const win = platform() == "windows";
    const nl = win ? "\r\n" : "\n";

    // ---------- 进程交互：cmd/sh 逐步回显 ----------
    const proc = win ? process_open("cmd", []) : process_open("sh", []);
    passed = passed & check(proc >= 0, "process_open");
    passed = passed & check(process_write(proc, "echo hello_from_sw" + nl) > 0, "process_write");

    // 逐行读直到找到关键字；空行/横幅/提示符都跳过，EOF 才停
    let found = false;
    let guard = 0;
    while (!found && guard < 200) {
        const status = process_poll(proc);
        if (status == 1) {
            const line = process_read_line(proc);
            if (contains(line, "hello_from_sw")) {
                found = true;
            }
        } else if (status == -1) {
            break;
        } else {
            sleep_ms(10);
        }
        guard++;
    }
    passed = passed & check(found, "process read_line echo");

    // 退出并等待回收
    process_write(proc, "exit" + nl);
    let exited = false;
    let guard2 = 0;
    while (!exited && guard2 < 200) {
        if (process_poll(proc) == -1) {
            exited = true;
        } else {
            sleep_ms(20);
            guard2++;
        }
    }
    const code = process_wait(proc);
    passed = passed & check(code == 0, "process_wait exit code");

    // ---------- 进程交互：sort（close_input 触发 EOF 输出） ----------
    const sorter = process_open("sort", []);
    passed = passed & check(sorter >= 0, "process_open sort");
    process_write(sorter, "banana" + nl + "apple" + nl + "cherry" + nl);
    process_close_input(sorter);
    let sorted = "";
    let guard3 = 0;
    while (guard3 < 200) {
        const status = process_poll(sorter);
        if (status == 1) {
            const chunk = process_read_some(sorter, 4096);
            if (chunk != "") {
                sorted = sorted + chunk;
            }
        } else if (status == -1) {
            break;
        } else {
            sleep_ms(10);
            guard3++;
        }
    }
    process_wait(sorter);
    passed = passed & check(
        contains(sorted, "apple") && contains(sorted, "banana") && contains(sorted, "cherry"),
        "process sort collects all"
    );
    const apple_index = index_of(sorted, "apple");
    const banana_index = index_of(sorted, "banana");
    passed = passed & check(apple_index >= 0 && banana_index > apple_index, "process sort order");

    // ---------- console 进度条（纯 Sw） ----------
    passed = passed & check(progress_text(50, 10) == "[#####-----] 50%", "progress_text 50");
    passed = passed & check(progress_text(0, 5) == "[-----] 0%", "progress_text 0");
    passed = passed & check(progress_text(100, 5) == "[#####] 100%", "progress_text 100");
    passed = passed & check(progress_text(-5, 10) == "[----------] 0%", "progress_text clamp low");
    passed = passed & check(progress_text(150, 4) == "[####] 100%", "progress_text clamp high");
    console_progress(30, 10);
    println("");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
