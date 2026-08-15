import { println, print, flush } from "std/io";
import {
    process_open,
    process_write,
    process_read_line,
    process_poll,
    process_wait,
    process_close_input,
    platform,
} from "std/os";
import { sleep_ms } from "std/time";
import { contains } from "std/string";
import { console_progress } from "std/console";
import { format } from "std/string";

// 进程交互演示：启动 cmd/sh 逐步对话 + sort 管道 + 动态进度条。
function main(): int {
    const win = platform() == "windows";
    const nl = win ? "\r\n" : "\n";

    println("== 交互式子进程（cmd/sh 对话） ==");
    const shell = win ? process_open("cmd", []) : process_open("sh", []);
    process_write(shell, "echo hello from subprocess" + nl);
    let found = false;
    let guard = 0;
    while (!found && guard < 200) {
        if (process_poll(shell) == 1) {
            const line = process_read_line(shell);
            if (contains(line, "hello from subprocess")) {
                found = true;
            }
        } else {
            sleep_ms(10);
        }
        guard++;
    }
    println(format("子进程回显: %s", found ? "收到" : "未收到"));
    process_write(shell, "exit" + nl);
    println(format("退出码=%d", process_wait(shell)));

    println("== sort 管道（close_input 触发 EOF 排序） ==");
    const sorter = process_open("sort", []);
    process_write(sorter, "banana" + nl + "apple" + nl + "cherry" + nl);
    process_close_input(sorter);
    let sorted = "";
    guard = 0;
    while (guard < 200) {
        if (process_poll(sorter) == 1) {
            const chunk = process_read_line(sorter);
            if (chunk == "") {
                break;
            }
            sorted = sorted + chunk + " ";
        } else if (process_poll(sorter) == -1) {
            break;
        } else {
            sleep_ms(10);
        }
        guard++;
    }
    process_wait(sorter);
    println(format("排序结果: %s", sorted));

    println("== 动态进度条 ==");
    let p = 0;
    while (p <= 100) {
        console_progress(p, 20);
        sleep_ms(5);
        p = p + 5;
    }
    println("");
    flush();
    println("完成。");
    return 0;
}
