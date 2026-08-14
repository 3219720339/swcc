// ===========================================================================
// std/os —— 进程与系统环境
//
// 用法：
//   import { getenv, exit, platform, run, run_status, spawn, wait, poll, kill } from "std/os";
//   const home = getenv("HOME");        // string?；未设置时为 null
//   exit(1);                            // 立即退出，返回码 1
//   const out = run("echo", ["hi"]);    // 执行到结束，返回 stdout+stderr 合并文本
//   const code = run_status("cmd", ["/c", "exit", "3"]);  // 只取退出码
//   const pid = spawn("sleep", ["5"]);  // 后台启动
//   wait(pid);                          // 阻塞等待并回收，返回退出码
//
// 命令行参数：把 main 声明为 function main(args: string[]): int，
// args 即参数数组（args[0] 为程序名）。运行：swc run file.sw a b c
// 或直接执行编译产物 a b c。
//
// 进程 API 说明：
//   - args 为 argv[1..]（argv[0] 即 cmd 本身）；命令名按 PATH 查找（execvp/CreateProcess）。
//   - run / run_with_input 返回捕获的 stdout+stderr 合并文本；启动失败返回空字符串。
//     run_with_input 的 input 建议不超过管道缓冲（Windows 4KB / POSIX 64KB），
//     过大且子进程不及时读取时会阻塞等待。
//   - run_status 不捕获输出（继承父进程的 stdout/stderr），返回退出码；
//     启动失败返回 -1；POSIX 上被信号杀死返回 128+信号号。
//   - spawn 失败返回 0（0 永远不是有效 pid）。
//   - wait(pid) 阻塞等待并回收，返回退出码；未知 pid 返回 -1。
//   - poll(pid) 非阻塞：返回退出码（已结束），-1 表示仍在运行，-2 表示未知 pid；
//     注意 poll 会回收进程，之后不要再 wait 同一个 pid。
//   - kill(pid) 强制终止：成功 0，失败 -1；随后可 wait 回收。
// ===========================================================================

/// 读取环境变量；不存在时返回 null。
export extern c function getenv(name: string): string?;

/// 立即终止进程并返回退出码。
export extern c function exit(code: int): void;

/// 当前操作系统："windows" / "linux" / "macos"。
export extern c function platform(): string;

/// 执行命令并等待结束，返回 stdout+stderr 合并文本；启动失败返回空字符串。
export extern c function run(cmd: string, args: string[]): string;

/// 执行命令、先写入 stdin（input）再等待结束，返回 stdout+stderr 合并文本。
export extern c function run_with_input(cmd: string, args: string[], input: string): string;

/// 执行命令并返回退出码（输出继承父进程，不捕获）；启动失败返回 -1。
export extern c function run_status(cmd: string, args: string[]): int;

/// 后台启动进程，返回 pid；失败返回 0。
export extern c function spawn(cmd: string, args: string[]): int;

/// 阻塞等待 pid 结束并回收，返回退出码；未知 pid 返回 -1。
export extern c function wait(pid: int): int;

/// 非阻塞查询：已结束返回退出码（进程已被回收），仍在运行返回 -1，未知 pid 返回 -2。
export extern c function poll(pid: int): int;

/// 强制终止 pid 对应的进程；成功返回 0，失败返回 -1。
export extern c function kill(pid: int): int;

/// 当前工作目录（绝对路径）；失败返回空字符串。
export extern c function cwd(): string;

/// 切换当前工作目录；成功返回 0，失败返回 -1。
export extern c function chdir(path: string): int;

/// 系统临时目录（如 /tmp 或 %TEMP%）；未知时返回空字符串。
export extern c function temp_dir(): string;

/// 当前用户主目录；未知时返回空字符串。
export extern c function home_dir(): string;

/// 本机主机名；失败返回空字符串。
export extern c function hostname(): string;

/// 逻辑 CPU 核心数（>= 1）。
export extern c function cpu_count(): int;

/// 全部环境变量名（不含值），顺序不保证。
export extern c function env_keys(): string[];

/// 设置环境变量（影响当前进程及其子进程）；成功返回 0，失败返回 -1。
export extern c function setenv(name: string, value: string): int;
