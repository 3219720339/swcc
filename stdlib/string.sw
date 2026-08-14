// ===========================================================================
// std/string —— 字符串处理
//
// 除下列自由函数外，字符串还支持链式方法调用：
//   "  hi  ".trim().to_upper()            // "HI"
//   "a,b".split(",").join("-")            // "a-b"
//   "42".parse_int()                      // 42
//   "abc".replace("b", "X")               // "aXc"
//   "hello".contains("ell")               // true
//
// 编码语义：
//   .length / utf8_len / s[i] 均按 Unicode 码点（UTF-8 字符）计；
//   index_of / substring 按字节偏移（v0.1 限制，处理 ASCII 无碍，
//   含中文时请用 utf8 系列函数）。
// ===========================================================================

/// 返回 needle 在 text 中首次出现的字节偏移；未找到返回 -1。
export extern c function index_of(text: string, needle: string): int;

/// text 是否包含 needle（true/false）。
export extern c function contains(text: string, needle: string): bool;

/// text 是否以 prefix 开头。
export extern c function starts_with(text: string, prefix: string): bool;

/// 截取 [start, start+length) 的字节片段；越界时自动裁剪。
export extern c function substring(text: string, start: int, length: int): string;

/// 转大写（仅 ASCII 字母）。
export extern c function to_upper(text: string): string;

/// 转小写（仅 ASCII 字母）。
export extern c function to_lower(text: string): string;

/// 去除首尾空白（空格/制表/换行/回车）。
export extern c function trim(text: string): string;

/// 按 separator 拆分（字节语义），返回 string[]；空分隔符返回单元素数组。
export extern c function split(text: string, separator: string): string[];

/// 用 separator 连接 string[]，返回拼接结果。
export extern c function join(items: string[], separator: string): string;

/// 把 text 中所有 from 替换为 to。
export extern c function replace(text: string, from: string, to: string): string;

/// 十进制字符串转整数；无法解析时返回 0。
export extern c function parse_int(text: string): int;

/// 十进制字符串转浮点；无法解析时返回 0.0。
export extern c function parse_float(text: string): float;
