import { println } from "std/io";
import {
    getenv,
    platform,
    run,
    run_with_input,
    run_status,
    spawn,
    wait,
    poll,
    kill,
    cwd,
    temp_dir,
    home_dir,
    hostname,
    cpu_count,
    env_keys,
    setenv,
    desktop_dir,
    documents_dir,
    downloads_dir,
    username,
    pid,
    arch,
    system_dir,
} from "std/os";

function main(): int {
    println(`platform=${platform()} arch=${arch()} pid=${pid()} cpu=${cpu_count()}`);
    println(`cwd=${cwd()}`);
    println(`home=${home_dir()} temp=${temp_dir()}`);
    println(`hostname=${hostname()} username=${username()}`);
    println(`desktop=${desktop_dir()} documents=${documents_dir()} downloads=${downloads_dir()}`);
    println(`system_dir=${system_dir()}`);
    const keys = env_keys();
    println(`env_keys_count=${keys.length}`);
    const path = getenv("PATH") ?? "(none)";
    println(`PATH_len=${path.length}`);
    setenv("SW_DEMO_VAR", "demo-value");
    println(`getenv_after_set=${getenv("SW_DEMO_VAR") ?? "(none)"}`);

    const os = platform();
    let out = "";
    if (os == "windows") {
        out = run("cmd", ["/c", "echo", "hello-from-subprocess"]);
    } else {
        out = run("echo", ["hello-from-subprocess"]);
    }
    println(`run=[${out.trim()}]`);
    let code = 0;
    if (os == "windows") {
        code = run_status("cmd", ["/c", "exit", "7"]);
    } else {
        code = run_status("sh", ["-c", "exit 7"]);
    }
    println(`run_status=${code}`);
    let pid2 = 0;
    if (os == "windows") {
        pid2 = spawn("cmd", ["/c", "exit", "5"]);
    } else {
        pid2 = spawn("sh", ["-c", "exit 5"]);
    }
    println(`spawn_pid=${pid2} wait=${wait(pid2)}`);
    return 0;
}
