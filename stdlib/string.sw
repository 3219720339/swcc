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

/// 字节数格式化：format_bytes(1536) == "1.5 KB"；支持 B/KB/MB/GB/TB。
export extern c function format_bytes(bytes: int): string;

/// 千分位格式化：format_thousands(1234567) == "1,234,567"。
export extern c function format_thousands(value: int): string;

/// 整数转十六进制（小写，无前缀）：int_to_hex(255) == "ff"。
export extern c function int_to_hex(value: int): string;

/// 整数转八进制（无前缀）：int_to_oct(8) == "10"。
export extern c function int_to_oct(value: int): string;

/// 整数转二进制（无前缀）：int_to_bin(5) == "101"。
export extern c function int_to_bin(value: int): string;

/// 按进制（2-36）解析字符串；非法返回 0。示例：parse_int_radix("ff", 16) == 255。
export extern c function parse_int_radix(text: string, radix: int): int;

/// 驼峰转蛇形：to_snake_case("helloWorld") == "hello_world"。
export extern c function to_snake_case(text: string): string;

/// 蛇形/空格/短横转驼峰：to_camel_case("hello_world") == "helloWorld"。
export extern c function to_camel_case(text: string): string;

/// 是否全部为字母（ASCII a-zA-Z，空串 false）。
export extern c function is_alpha(text: string): bool;

/// 是否全部为字母或数字（ASCII，空串 false）。
export extern c function is_alnum(text: string): bool;

/// 是否全部为标点（ASCII 可见非字母数字，空串 false）。
export extern c function is_punct(text: string): bool;

// ---------------------------------------------------------------------------
// 中文函数名（转发到英文实现，火山风格命名；实际符号仍是英文函数）
// ---------------------------------------------------------------------------

export function 反转文本(text: string): string {
    return reverse(text);
}

export function 替换文本(text: string, from: string, to: string): string {
    return replace(text, from, to);
}

export function 分割文本(text: string, separator: string): string[] {
    return split(text, separator);
}

export function 是否为空(text: string): bool {
    return is_empty(text);
}

export function 是否空白(text: string): bool {
    return is_blank(text);
}

export function 删全部空白(text: string): string {
    return strip_whitespace(text);
}

export function 取文本中间(text: string, start: string, end: string): string {
    return substring_between(text, start, end);
}

export function 取文本中间反向(text: string, start: string, end: string): string {
    return substring_between_last(text, start, end);
}

export function 批量取文本中间(text: string, start: string, end: string): string[] {
    return extract_between(text, start, end);
}

export function 取文本左边(text: string, marker: string): string {
    return before(text, marker);
}

export function 取文本右边(text: string, marker: string): string {
    return after(text, marker);
}

export function 取文本左边反向(text: string, marker: string): string {
    return before_last(text, marker);
}

export function 取文本右边反向(text: string, marker: string): string {
    return after_last(text, marker);
}

export function 取字符代码(text: string, index: int): int {
    return char_code(text, index);
}

export extern c function 连续子文本替换(text: string, ...pairs: any): string;

export function 转大写(text: string): string {
    return to_upper(text);
}

export function 转小写(text: string): string {
    return to_lower(text);
}

export function 首字母大写(text: string): string {
    return capitalize(text);
}

export function 出现次数(text: string, needle: string): int {
    return count(text, needle);
}

export function 是否包含(text: string, needle: string): bool {
    return contains(text, needle);
}

export function 开头为(text: string, prefix: string): bool {
    return starts_with(text, prefix);
}

export function 结尾为(text: string, suffix: string): bool {
    return ends_with(text, suffix);
}

export function 删除前缀(text: string, prefix: string): string {
    return remove_prefix(text, prefix);
}

export function 删除后缀(text: string, suffix: string): string {
    return remove_suffix(text, suffix);
}

export function 字节数格式化(bytes: int): string {
    return format_bytes(bytes);
}

export function 千分位格式化(value: int): string {
    return format_thousands(value);
}

export function 整数转十六进制(value: int): string {
    return int_to_hex(value);
}

export function 整数转八进制(value: int): string {
    return int_to_oct(value);
}

export function 整数转二进制(value: int): string {
    return int_to_bin(value);
}

export function 按进制解析整数(text: string, radix: int): int {
    return parse_int_radix(text, radix);
}

export function 转蛇形命名(text: string): string {
    return to_snake_case(text);
}

export function 转驼峰命名(text: string): string {
    return to_camel_case(text);
}

export function 是否字母(text: string): bool {
    return is_alpha(text);
}

export function 是否字母数字(text: string): bool {
    return is_alnum(text);
}

export function 是否标点(text: string): bool {
    return is_punct(text);
}

/// 模板渲染：把 text 中的 {key} 占位符替换为 map 中对应值；
/// {{ 转义为字面 {；未知键替换为空串。
/// 示例：render_template("Hi {name}, n={n}", map)。
export extern c function render_template(text: string, map: ptr<void>): string;

export function 模板渲染(text: string, map: ptr<void>): string {
    return render_template(text, map);
}

/// 文本转 URL 友好 slug：小写、字母数字保留（中文保留）、其余转 '-'
/// 并压缩连续 '-'、去首尾。示例：slugify("Hello World!") == "hello-world"。
export extern c function slugify(text: string): string;

export function 转网址别名(text: string): string {
    return slugify(text);
}

/// 只替换首次出现的 from（未找到原样返回）。字节语义同 replace。
export function replace_first(text: string, from: string, to: string): string {
    const idx = index_of(text, from);
    if (idx < 0) {
        return text;
    }
    return before(text, from) + to + after(text, from);
}

/// 只替换最后一次出现的 from（未找到原样返回）。
export function replace_last(text: string, from: string, to: string): string {
    const idx = last_index_of(text, from);
    if (idx < 0) {
        return text;
    }
    return before_last(text, from) + to + after_last(text, from);
}

/// 按首个 separator 拆成两段，返回 string[2]（[左侧, 右侧]）；
/// 未找到返回 [text, ""]。
export function split_once(text: string, separator: string): string[] {
    if (index_of(text, separator) < 0) {
        return [text, ""];
    }
    return [before(text, separator), after(text, separator)];
}

/// 按空白分词计数：count_words("hello  world") == 2。
export function count_words(text: string): int {
    return split_whitespace(text).length;
}

/// 居中填充到指定字符宽度（pad 取首个字符；width 不足时不填充）。
export function pad_center(text: string, width: int, pad: string): string {
    const total = width - text.length;
    if (total <= 0) {
        return text;
    }
    const left = total / 2;
    const right = total - left;
    return pad_right(pad_left(text, text.length + left, pad), width, pad);
}

/// 最长公共前缀（按字符比较，UTF-8 安全）。
export function common_prefix(a: string, b: string): string {
    const n = a.length < b.length ? a.length : b.length;
    let result = "";
    let i = 0;
    while (i < n && char_code(a, i) == char_code(b, i)) {
        result = result + from_code_point(char_code(a, i));
        i++;
    }
    return result;
}

/// 最长公共后缀（按字符比较，UTF-8 安全）。
export function common_suffix(a: string, b: string): string {
    const n = a.length < b.length ? a.length : b.length;
    let result = "";
    let i = 0;
    while (i < n && char_code(a, a.length - 1 - i) == char_code(b, b.length - 1 - i)) {
        result = from_code_point(char_code(a, a.length - 1 - i)) + result;
        i++;
    }
    return result;
}

/// 大小写互换：swap_case("Hello123") == "hELLO123"。
export function swap_case(text: string): string {
    let result = "";
    const arr = chars(text);
    for (const c of arr) {
        if (is_upper(c)) {
            result = result + to_lower(c);
        } else if (is_lower(c)) {
            result = result + to_upper(c);
        } else {
            result = result + c;
        }
    }
    return result;
}

/// 数字字符串前补零到指定宽度：zfill("42", 5) == "00042"。
export function zfill(text: string, width: int): string {
    if (text.length >= width) {
        return text;
    }
    return repeat("0", width - text.length) + text;
}

export function 替换首次出现(text: string, from: string, to: string): string {
    return replace_first(text, from, to);
}

export function 替换最后出现(text: string, from: string, to: string): string {
    return replace_last(text, from, to);
}

export function 拆分一次(text: string, separator: string): string[] {
    return split_once(text, separator);
}

export function 取单词数(text: string): int {
    return count_words(text);
}

export function 居中填充(text: string, width: int, pad: string): string {
    return pad_center(text, width, pad);
}

export function 取公共前缀(a: string, b: string): string {
    return common_prefix(a, b);
}

export function 取公共后缀(a: string, b: string): string {
    return common_suffix(a, b);
}

export function 大小写互换(text: string): string {
    return swap_case(text);
}

export function 前补零(text: string, width: int): string {
    return zfill(text, width);
}

/// 取第 index 个字符（UTF-8 码点）；越界返回空串。
export function char_at(text: string, index: int): string {
    const code = char_code(text, index);
    return code < 0 ? "" : from_code_point(code);
}

/// 取前 n 个字符（UTF-8 码点，越界自动裁剪）。left("hello", 2) == "he"。
export function left(text: string, n: int): string {
    const count = n < 0 ? 0 : (n > text.length ? text.length : n);
    return truncate(text, count);
}

/// 取后 n 个字符（UTF-8 码点，越界自动裁剪）。right("hello", 2) == "lo"。
export function right(text: string, n: int): string {
    const arr = chars(text);
    const count = n < 0 ? 0 : (n > arr.length ? arr.length : n);
    let result = "";
    let i = arr.length - count;
    while (i < arr.length) {
        result = result + arr[i];
        i++;
    }
    return result;
}

/// text 是否以 prefixes 中任意一个开头（文件扩展名/URL 白名单判断常用）。
export function starts_with_any(text: string, prefixes: string[]): bool {
    for (const prefix of prefixes) {
        if (starts_with(text, prefix)) {
            return true;
        }
    }
    return false;
}

/// text 是否以 suffixes 中任意一个结尾。
export function ends_with_any(text: string, suffixes: string[]): bool {
    for (const suffix of suffixes) {
        if (ends_with(text, suffix)) {
            return true;
        }
    }
    return false;
}

/// 莱文斯坦编辑距离（字节级 DP；模糊搜索/纠错提示用）。
/// 示例：edit_distance("kitten", "sitting") == 3。
export extern c function edit_distance(a: string, b: string): int;

export function 取字符文本(text: string, index: int): string {
    return char_at(text, index);
}

export function 取左边文本(text: string, n: int): string {
    return left(text, n);
}

export function 取右边文本(text: string, n: int): string {
    return right(text, n);
}

export function 开头为任意(text: string, prefixes: string[]): bool {
    return starts_with_any(text, prefixes);
}

export function 结尾为任意(text: string, suffixes: string[]): bool {
    return ends_with_any(text, suffixes);
}

export function 编辑距离(a: string, b: string): int {
    return edit_distance(a, b);
}
