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

/// 按行读取整个文件为 string[]（去掉行尾 \n / \r，末尾换行不产生空行）。
export extern c function read_lines(path: string): string[];

/// 覆盖写入整个文件（不存在则创建）；返回写入字节数，失败返回 -1。
export extern c function write_all(path: string, text: string): int;

/// 追加写入文件（不存在则创建）；返回写入字节数，失败返回 -1。
export extern c function append(path: string, text: string): int;

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

/// 取路径最后一段（文件名）。
export extern c function path_basename(path: string): string;

/// 取路径的目录部分；无分隔符时返回 "."。
export extern c function path_dirname(path: string): string;

/// 取扩展名（含点，如 ".txt"）；无扩展名返回空字符串。
export extern c function path_ext(path: string): string;

/// 列出目录内容（文件名/子目录名，不含 "." 与 ".."）；目录不存在返回空数组。
export extern c function list_dir(path: string): string[];

/// 判断路径是否为目录：1 是 / 0 否（不存在也返回 0）。
export extern c function is_dir(path: string): int;

/// 创建目录（不递归，父目录必须存在）；成功返回 0，失败返回 -1。
export extern c function mkdir(path: string): int;

/// 删除文件（或空目录）；成功返回 0，失败返回 -1。
export extern c function remove(path: string): int;

/// 重命名/移动文件；成功返回 0，失败返回 -1。
export extern c function rename(old_path: string, new_path: string): int;

/// 复制文件；返回复制的字节数，失败返回 -1。
export extern c function copy_file(src: string, dst: string): int;

/// 递归收集 path 下所有文件（完整路径，含子目录）。
export extern c function walk_files(path: string): string[];

/// 递归收集 path 下所有目录（完整路径，含子目录）。
export extern c function walk_dirs(path: string): string[];

/// 读取整个文件为 u8[]（紧凑字节布局）；文件不存在返回空数组。
export extern c function read_file_bytes(path: string): u8[];

/// 把 u8[] 原样写入文件（不存在则创建）；返回写入字节数，失败返回 -1。
export extern c function write_file_bytes(path: string, bytes: u8[]): int;

/// 文件大小（字节）；按路径打开读取，失败（如目录/不存在）返回 -1。
/// 注意与 file_size(fd) 区分：本函数直接接受路径。
export extern c function file_size_path(path: string): int;

/// 文件最后修改时间（Unix 秒）；失败返回 -1。
export extern c function file_mtime(path: string): int;

/// 判断路径是否为普通文件（非目录）：1 是 / 0 否（不存在也返回 0）。
export extern c function is_file(path: string): int;

/// 修改文件权限（mode 为 POSIX 风格八进制值，如 0o644 传 420）；
/// Windows 上仅区分只读（无写位）与可写。成功返回 0，失败返回 -1。
export extern c function chmod(path: string, mode: int): int;

/// 创建空文件（已存在则更新时间戳）；成功返回 0，失败返回 -1。
export extern c function touch(path: string): int;

/// 递归复制目录（含子目录与文件，不含符号链接）；成功返回 0，失败返回 -1。
export extern c function copy_dir(src: string, dst: string): int;

/// 递归删除文件或目录（危险：不可恢复）；成功返回 0，失败返回 -1。
export extern c function remove_all(path: string): int;

/// 通配匹配文件路径，支持 * 与 ?（作用于文件名部分）；
/// 返回匹配项的完整路径，无匹配返回空数组。
export extern c function glob(pattern: string): string[];
