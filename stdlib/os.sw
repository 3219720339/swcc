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

/// 删除环境变量；成功返回 0，失败返回 -1。
export extern c function unsetenv(name: string): int;

/// 桌面目录（Windows 已知文件夹 / XDG 或 $HOME/Desktop）。
export extern c function desktop_dir(): string;

/// 文档目录（XDG 或 $HOME/Documents）。
export extern c function documents_dir(): string;

/// 下载目录（XDG 或 $HOME/Downloads）。
export extern c function downloads_dir(): string;

/// 图片目录（XDG 或 $HOME/Pictures）。
export extern c function pictures_dir(): string;

/// 音乐目录（XDG 或 $HOME/Music）。
export extern c function music_dir(): string;

/// 视频目录（XDG 或 $HOME/Videos）。
export extern c function videos_dir(): string;

/// 配置目录（Windows %APPDATA% / XDG 或 $HOME/.config）。
export extern c function config_dir(): string;

/// 系统目录（Windows System32 / macOS /System / Linux /usr）。
export extern c function system_dir(): string;

/// 当前用户名（Windows USERNAME / POSIX USER 或 LOGNAME）；未知返回空串。
export extern c function username(): string;

/// 当前进程 ID。
export extern c function pid(): int;

/// 机器架构："x86_64" / "aarch64"。
export extern c function arch(): string;

/// 命令行参数中是否存在指定 flag：等于 name 或形如 name=value。
/// 示例：flag_has(args, "--verbose")、flag_has(args, "-v")。
export extern c function flag_has(args: string[], name: string): bool;

/// 取 flag 的值：支持 "--name=value" 与 "--name value" 两种写法；
/// 未提供返回 null。示例：flag_value(args, "--port") == "8080"。
export extern c function flag_value(args: string[], name: string): string?;

// ---------------------------------------------------------------------------
// 中文函数名（转发到英文实现，火山风格命名）
// ---------------------------------------------------------------------------

export function 含参数(args: string[], name: string): bool {
    return flag_has(args, name);
}

export function 取参数值(args: string[], name: string): string? {
    return flag_value(args, name);
}

/// 按 PATH 查找可执行文件完整路径；未找到返回空串。
/// 示例：os_which("cmd")、os_which("python")。
export extern c function os_which(name: string): string;

/// 创建唯一临时目录，返回完整路径；失败返回空串。
export extern c function mkdtemp(prefix: string): string;

export function 查找可执行文件(name: string): string {
    return os_which(name);
}

export function 创建临时目录(prefix: string): string {
    return mkdtemp(prefix);
}

/// 带环境变量执行命令并等待结束，返回 stdout+stderr 合并文本。
/// env 为 map（字符串键值，string 值）；启动失败返回空字符串。
export extern c function run_with_env(cmd: string, args: string[], env: ptr<void>): string;

/// 在指定工作目录执行命令并等待结束，返回 stdout+stderr 合并文本。
export extern c function run_in_dir(cmd: string, args: string[], dir: string): string;

/// 执行命令并分别捕获 stdout 与 stderr，返回 string[2]（[stdout, stderr]）。
/// 注意：输出超过管道缓冲（Windows 4KB / POSIX 64KB）时可能阻塞。
export extern c function run_stdout_stderr(cmd: string, args: string[]): string[];

/// pid 对应进程是否仍在运行（自身权限范围内）。
export extern c function is_process_running(pid: int): bool;

export function 带环境运行(cmd: string, args: string[], env: ptr<void>): string {
    return run_with_env(cmd, args, env);
}

export function 在目录运行(cmd: string, args: string[], dir: string): string {
    return run_in_dir(cmd, args, dir);
}

export function 分开捕获输出(cmd: string, args: string[]): string[] {
    return run_stdout_stderr(cmd, args);
}

export function 进程是否运行(pid: int): bool {
    return is_process_running(pid);
}

/// 当前进程内存占用（KB）；失败返回 -1。
export extern c function memory_usage_kb(): int;

export function 取内存占用(): int {
    return memory_usage_kb();
}
