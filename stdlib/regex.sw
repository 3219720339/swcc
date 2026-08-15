// ===========================================================================
// std/regex —— 正则表达式（最小引擎，纯运行时）
//
// 用法：
//   import { regex_match, regex_find, regex_find_all, regex_replace } from "std/regex";
//   regex_match("2026-08-15", "\\d{4}-\\d{2}-\\d{2}")   // true
//   regex_find("订单 #1024 已发货", "\\d+")              // "1024"
//   regex_find_all("a1 b22 c333", "\\d+")                // ["1","22","333"]
//   regex_replace("a1b2", "\\d", "#")                    // "a#b#"
//
// 支持语法：
//   字面量、. * + ?、[] 字符类（范围 a-z、取反 [^...]）、^ $ 锚点、
//   () 捕获分组、| 交替、\d \w \s \D \W \S、转义字符（\. \* 等）。
// 文本按 Unicode 码点处理，中文安全。
// ===========================================================================

/// 整个文本是否完全匹配模式（bool）。示例：regex_match("abc", "a.c") == true。
export extern c function regex_match(text: string, pattern: string): bool;

/// 返回第一个匹配的子串；无匹配返回空字符串。
/// 示例：regex_find("订单 #1024", "\\d+") == "1024"。
export extern c function regex_find(text: string, pattern: string): string;

/// 返回所有匹配的子串（非重叠，string[]）。
/// 示例：regex_find_all("a1 b22", "\\d+") == ["1","22"]。
export extern c function regex_find_all(text: string, pattern: string): string[];

/// 把 text 中所有匹配替换为 replacement。
/// $0 表示整个匹配；$1..$9 分组引用当前按字面输出（最小引擎暂未展开捕获）。
/// 示例：regex_replace("2026-08-15", "-", "/") == "2026/08/15"。
export extern c function regex_replace(text: string, pattern: string, replacement: string): string;

// ---------------------------------------------------------------------------
// 中文函数名（转发到英文实现，火山风格命名）
// ---------------------------------------------------------------------------

export function 正则匹配(text: string, pattern: string): bool {
    return regex_match(text, pattern);
}

export function 正则查找(text: string, pattern: string): string {
    return regex_find(text, pattern);
}

export function 正则查找全部(text: string, pattern: string): string[] {
    return regex_find_all(text, pattern);
}

export function 正则替换(text: string, pattern: string, replacement: string): string {
    return regex_replace(text, pattern, replacement);
}

/// 按模式匹配位置拆分：regex_split("a,b;c", "[,;]") == ["a","b","c"]。
export extern c function regex_split(text: string, pattern: string): string[];

/// 转义正则元字符（\. ^ $ * + ? { } [ ] ( ) | 前加反斜杠），用于把用户输入
/// 当字面量匹配。示例：regex_match("a.b", regex_escape("a.b")) == true。
export extern c function regex_escape(text: string): string;

/// 提取第一个匹配的捕获组（string[]，[0] 是整个匹配，[1..] 各组；
/// 未参与匹配的组返回空串）。
/// 示例：regex_captures("2026-08-15", "(\\d+)-(\\d+)-(\\d+)")
///       == ["2026-08-15", "2026", "08", "15"]。
export extern c function regex_captures(text: string, pattern: string): string[];

export function 正则拆分(text: string, pattern: string): string[] {
    return regex_split(text, pattern);
}

export function 正则转义(text: string): string {
    return regex_escape(text);
}

export function 正则捕获组(text: string, pattern: string): string[] {
    return regex_captures(text, pattern);
}
