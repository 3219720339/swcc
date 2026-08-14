// ===========================================================================
// std/io —— 控制台输入输出
//
// 用法：
//   import { println, print, read_line } from "std/io";
//   println("hello");          // 输出并换行
//   print("no newline");       // 输出不换行
//   const line = read_line();  // 从 stdin 读一行（去掉行尾换行符）
//
// 注意：read_line 读取长度上限 4095 字节；读到 EOF 返回空字符串。
// ===========================================================================

/// 输出一行文本（自动追加换行）。text 为空字符串时只输出换行。
export extern c function println(text: string): void;

/// 输出文本，不追加换行。
export extern c function print(text: string): void;

/// 从标准输入读取一行（UTF-8 文本），去掉行尾 \n 与 \r。
/// 返回 string；EOF 或失败时返回空字符串。
export extern c function read_line(): string;
