import { println } from "std/io";
import {
    platform,
    run,
    run_with_input,
    run_status,
    spawn,
    wait,
    poll,
    kill,
} from "std/os";
import { sleep_ms } from "std/time";

function main(): int {
    const os = platform();
    println(`os=${os}`);

    // 1) run：捕获 stdout+stderr 合并文本
    let out = "";
    if (os == "windows") {
        out = run("cmd", ["/c", "echo", "hello"]);
    } else {
        out = run("echo", ["hello"]);
    }
    const run1 = out;
    println(`run=[${out}]`);

    // 2) run_with_input：写 stdin 后捕获输出
    if (os == "windows") {
        out = run_with_input("cmd", ["/c", "more"], "piped-input\n");
    } else {
        out = run_with_input("cat", [], "piped-input\n");
    }
    const run2 = out;
    println(`input=[${out}]`);

    // 3) run_status：退出码
    let code = 0;
    if (os == "windows") {
        code = run_status("cmd", ["/c", "exit", "7"]);
    } else {
        code = run_status("sh", ["-c", "exit 7"]);
    }
    println(`status=${code}`);

    // 4) spawn + wait：退出码
    let pid = 0;
    if (os == "windows") {
        pid = spawn("cmd", ["/c", "exit", "5"]);
    } else {
        pid = spawn("sh", ["-c", "exit 5"]);
    }
    const waited = wait(pid);
    println(`wait=${waited}`);

    // 5) spawn + kill + wait 回收
    let pid2 = 0;
    if (os == "windows") {
        pid2 = spawn("ping", ["-n", "30", "127.0.0.1"]);
    } else {
        pid2 = spawn("sleep", ["30"]);
    }
    const killed = kill(pid2);
    wait(pid2);
    println(`kill=${killed}`);

    // 6) poll：等快速退出的进程结束
    let pid3 = 0;
    if (os == "windows") {
        pid3 = spawn("cmd", ["/c", "exit", "0"]);
    } else {
        pid3 = spawn("true", []);
    }
    let p = -1;
    let guard = 0;
    while (guard < 200) {
        p = poll(pid3);
        if (p != -1) {
            break;
        }
        sleep_ms(5);
        guard = guard + 1;
    }
    println(`poll=${p}`);

    let ok = 1;
    if (run1.index_of("hello") < 0) {
        ok = 0;
    }
    if (run2.index_of("piped-input") < 0) {
        ok = 0;
    }
    if (code != 7) {
        ok = 0;
    }
    if (waited != 5) {
        ok = 0;
    }
    if (killed != 0) {
        ok = 0;
    }
    if (p != 0) {
        ok = 0;
    }
    println(`ok=${ok}`);
    return ok == 1 ? 0 : 1;
}
