// ===========================================================================
// std/fs —— 文件读写与路径工具
//
// 用法：
//   import { open, close, write, read_all, exists, path_join } from "std/fs";
//   const fd = open("a.txt", "w");   // 写模式打开，失败返回 -1
//   write(fd, "hello");
//   close(fd);
//   const text = read_all("a.txt");  // 读整个文件为 string
//   const ok = exists("a.txt");      // 1 存在 / 0 不存在
//
// 文件描述符：open 返回 0~63 的表索引，最大同时打开 64 个文件。
// mode 取值： "r" 读 / "w" 写（清空）/ "a" 追加 / "r+" 读写 / "rb" 等二进制变体。
// 注意：write 写入的是字节（UTF-8），返回实际写入字节数。
// ===========================================================================

/// 打开文件，返回文件描述符（0~63）；失败返回 -1。
/// mode： "r" / "w" / "a" / "r+" / "rb" 等。
export extern c function open(path: string, mode: string): int;

/// 关闭文件描述符；成功返回 0，无效 fd 返回 -1。
export extern c function close(fd: int): int;

/// 向文件写入文本，返回写入字节数；失败返回 -1。
export extern c function write(fd: int, text: string): int;

/// 一次性读取整个文件为字符串；文件不存在返回空字符串。
export extern c function read_all(path: string): string;

/// 从文件按行读取（不含行尾 \n / \r）；EOF 返回空字符串。
export extern c function read_line_from(fd: int): string;

/// 移动文件读写位置。origin：0=文件头 / 1=当前位置 / 2=文件尾。
/// 成功返回 0，失败返回 -1。
export extern c function seek(fd: int, offset: int, origin: int): int;

/// 返回文件当前大小（字节）；无效 fd 返回 -1。会重置位置到文件头。
export extern c function file_size(fd: int): int;

/// 判断路径是否存在：1 存在 / 0 不存在。
export extern c function exists(path: string): int;

/// 拼接两个路径段，自动补平台分隔符（Windows 用 \，其余用 /）。
export extern c function path_join(a: string, b: string): string;
