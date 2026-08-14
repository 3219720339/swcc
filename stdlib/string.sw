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

/// 解析布尔：true/1/yes → true，false/0/no → false（不区分大小写），否则 false。
export extern c function parse_bool(text: string): bool;

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

/// text 是否以 suffix 结尾。
export extern c function ends_with(text: string, suffix: string): bool;

/// 去除左侧空白（空格/制表/换行/回车）。
export extern c function trim_left(text: string): string;

/// 去除右侧空白。
export extern c function trim_right(text: string): string;

/// 按行拆分（\n 分隔，兼容 \r\n）；末尾换行不产生空行，空字符串返回空数组。
export extern c function lines(text: string): string[];

/// 按连续空白（空格/制表/换行/回车）拆分，空白段不产生空元素。
export extern c function split_whitespace(text: string): string[];

/// 统计 needle 在 text 中的非重叠出现次数；needle 为空返回 0。
export extern c function count(text: string, needle: string): int;

/// 返回 needle 在 text 中最后一次出现的字节偏移；未找到返回 -1。
export extern c function last_index_of(text: string, needle: string): int;

/// 按字符（码点）拆成单个字符的 string[]，如 "你好" -> ["你","好"]。
export extern c function chars(text: string): string[];

/// 把 u8[]（UTF-8 字节）按原样转为字符串（不做合法性校验）。
export extern c function from_utf8_bytes(bytes: u8[]): string;

/// 把字符串按 UTF-8 字节转为 u8[]。
export extern c function to_utf8_bytes(text: string): u8[];

/// 是否全部为 ASCII（每个字节 < 0x80）。
export extern c function is_ascii(text: string): bool;

/// C 风格转义：\" \\ \n \r \t 与控制字符（\xNN）；unescape 可逆。
export extern c function escape(text: string): string;

/// 反转义 escape 的输出（支持 \n \r \t \" \\ \xNN）；非法转义按字面保留。
export extern c function unescape(text: string): string;

/// 是否为空字符串（长度为 0）。
export extern c function is_empty(text: string): bool;

/// 是否为合法 UTF-8 字节序列。
export extern c function utf8_is_valid(text: string): bool;

/// 按字符（码点）截断到最多 max_chars 个字符；不足则原样返回。
export extern c function truncate(text: string, max_chars: int): string;

/// 截断并追加 "..."（最多 max_chars 个字符）；不足则原样返回。
export extern c function ellipsis(text: string, max_chars: int): string;

/// 去掉前缀；不以该前缀开头则原样返回。
export extern c function remove_prefix(text: string, prefix: string): string;

/// 去掉后缀；不以该后缀结尾则原样返回。
export extern c function remove_suffix(text: string, suffix: string): string;

/// 是否全部为大写字母（非字母字符忽略）。
export extern c function is_upper(text: string): bool;

/// 是否全部为小写字母（非字母字符忽略）。
export extern c function is_lower(text: string): bool;

/// 是否全部为数字。
export extern c function is_digit(text: string): bool;

/// 首字母大写，其余不变。
export extern c function capitalize(text: string): string;

/// 是否空白（空格/全角空格/换行/制表符，空串也算空白）。
export extern c function is_blank(text: string): bool;

/// 删除全部空白（空格、全角空格、\r \n \t）。
export extern c function strip_whitespace(text: string): string;

/// 取首个 start 与之后首个 end 之间的内容；找不到返回空串。
export extern c function substring_between(text: string, start: string, end: string): string;

/// 从右往左：最后一个 end 之前、最后一个 start 之后的内容。
export extern c function substring_between_last(text: string, start: string, end: string): string;

/// 批量提取 start 与 end 之间的全部内容，返回 string[]。
export extern c function extract_between(text: string, start: string, end: string): string[];

/// 第一个 marker 左侧的内容；找不到返回空串。
export extern c function before(text: string, marker: string): string;

/// 第一个 marker 右侧的内容；找不到返回空串。
export extern c function after(text: string, marker: string): string;

/// 最后一个 marker 左侧的内容；找不到返回空串。
export extern c function before_last(text: string, marker: string): string;

/// 最后一个 marker 右侧的内容；找不到返回空串。
export extern c function after_last(text: string, marker: string): string;

/// 第 index 个字符（UTF-8 码点）的代码值；越界返回 -1。
export extern c function char_code(text: string, index: int): int;

/// 连续子文本替换：参数成对（欲替换值, 替换值, ...），依次替换。
/// 例：replace_pairs("你好，火山", "你好", "Hello", "火山", "火山中文编程")
///     → "Hello，火山中文编程"
export extern c function replace_pairs(text: string, ...pairs: any): string;
