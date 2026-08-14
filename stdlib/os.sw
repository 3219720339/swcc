// ===========================================================================
// std/os —— 进程与系统环境
//
// 用法：
//   import { getenv, exit } from "std/os";
//   const home = getenv("HOME");        // string?；未设置时为 null
//   exit(1);                            // 立即退出，返回码 1
//
// 命令行参数：把 main 声明为 function main(args: string[]): int，
// args 即参数数组（args[0] 为程序名）。运行：swc run file.sw a b c
// 或直接执行编译产物 a b c。
// ===========================================================================

/// 读取环境变量；不存在时返回 null。
export extern c function getenv(name: string): string?;

/// 立即终止进程并返回退出码。
export extern c function exit(code: int): void;
