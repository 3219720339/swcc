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

/// printf 风格格式化：%d/%i/%u/%x/%X/%o/%f/%e/%g/%s/%c/%%。
/// 支持宽度与精度（如 %5d、%.2f、%-10s、%08x）；参数按顺序消费，
/// 少传用 0/空字符串补齐，多传忽略。示例：
///   format("%s: %d (%.2f)", "score", 42, 3.14159)  // "score: 42 (3.14)"
export extern c function format(fmt: string, ...args: any): string;

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

/// 判断字符串是否为合法数字（整数或浮点，支持 +/- 与指数）。
export extern c function is_number(text: string): bool;

/// 解析整数；无法解析时返回 fallback（显式错误处理）。
export extern c function parse_int_or(text: string, fallback: int): int;

/// 解析浮点；无法解析时返回 fallback。
export extern c function parse_float_or(text: string, fallback: float): float;

/// 把字符串重复 count 次拼接（count <= 0 返回空串）。
export extern c function repeat(text: string, count: int): string;

/// 把 Unicode 码点编码为单个字符（UTF-8）。
export extern c function from_code_point(code_point: int): string;

/// 按字符（码点）反转字符串，如 "你好a" -> "a好你"。
export extern c function reverse(text: string): string;

/// 按字符（码点）查找 needle 首次出现的字符序号；未找到返回 -1。
export extern c function index_of_char(text: string, needle: string): int;

/// 按字符拆分（有效 UTF-8 分隔符天然落在字符边界）。
export extern c function split_chars(text: string, separator: string): string[];

/// 左侧填充到指定字符宽度（pad 取首个字符；width 不足时不填充）。
export extern c function pad_left(text: string, width: int, pad: string): string;

/// 右侧填充到指定字符宽度。
export extern c function pad_right(text: string, width: int, pad: string): string;

/// 格式化整数：pad_zero 为 1 时补零，为 0 时右对齐补空格。
/// 示例：format_int(42, 3, 1) == "042"；format_int(42, 3, 0) == " 42"。
export extern c function format_int(value: int, width: int, pad_zero: int): string;

/// 格式化浮点：保留 precision 位小数，如 format_float(3.14159, 2) == "3.14"。
export extern c function format_float(value: float, precision: int): string;
