import { println } from "std/io";
import {
    format,
    contains,
    starts_with,
    substring,
    to_upper,
    to_lower,
    trim,
    split,
    join,
    replace,
    parse_int,
    parse_float,
    is_number,
    parse_bool,
    parse_int_or,
    repeat,
    from_code_point,
    reverse,
    index_of_char,
    split_chars,
    pad_left,
    pad_right,
    format_int,
    format_float,
    ends_with,
    trim_left,
    trim_right,
    lines,
    split_whitespace,
    count,
    last_index_of,
    chars,
    is_ascii,
    escape,
    unescape,
    is_empty,
    utf8_is_valid,
    truncate,
    ellipsis,
    remove_prefix,
    remove_suffix,
    is_upper,
    is_lower,
    is_digit,
    capitalize,
    is_blank,
    strip_whitespace,
    substring_between,
    substring_between_last,
    extract_between,
    before,
    after,
    before_last,
    after_last,
    char_code,
    replace_pairs,
    反转文本,
    是否空白,
    删全部空白,
    取文本中间,
    取文本左边,
    取文本右边,
    是否包含,
    开头为,
    结尾为,
    出现次数,
    首字母大写,
    删除前缀,
} from "std/string";

function join_all(items: string[], sep: string): string {
    return join(items, sep);
}

function main(): int {
    const s = "  Hello, Sw  ";
    println(`trim=[${trim(s)}] upper=${to_upper(s)} lower=${to_lower(s)}`);
    println(`split-join=${join_all(split(trim(s), " "), "|")}`);
    println(`replace=${replace("a-b-c", "-", "+")} count=${count("ababa", "ab")}`);
    println(`contains=${contains("hello", "ell")} starts=${starts_with("hello", "he")} ends=${ends_with("hello", "lo")}`);
    println(`substring=${substring("hello", 1, 3)} lines=${lines("a\nb\nc").length}`);
    println(`parse_int=${parse_int("42")} parse_float=${format_float(parse_float("3.14"), 2)}`);
    println(`parse_int_or=${parse_int_or("abc", -1)} is_number=${is_number("3.14")} parse_bool=${parse_bool("yes")}`);
    println(`format=${format("%s: %d (%.2f) %08x", "score", 42, 3.14159, 255)}`);
    println(`repeat=${repeat("-=", 3)} from_code_point=${from_code_point(20320)}`);
    println(`reverse=${reverse("你好Sw")} index_of_char=${index_of_char("你好Sw", "Sw")}`);
    println(`split_chars=${join_all(split_chars("你好", ","), ",")} chars=${join_all(chars("你好"), ",")}`);
    println(`pad_left=${pad_left("42", 5, "0")} pad_right=${pad_right("42", 5, "-")}`);
    println(`format_int=${format_int(42, 3, 1)} format_float=${format_float(3.14159, 2)}`);
    println(`trim_left=[${trim_left("  hi  ")}] trim_right=[${trim_right("  hi  ")}]`);
    println(`split_whitespace=${split_whitespace(" a  b c ").length} last_index_of=${last_index_of("ababa", "ab")}`);
    println(`is_ascii=${is_ascii("hello")} is_ascii_cn=${is_ascii("你好")} is_empty=${is_empty("")}`);
    println(`escape=${escape("a\"b\nc")} unescape=${unescape("a\\\"b\\nc")}`);
    println(`utf8_is_valid=${utf8_is_valid("你好")} truncate=${truncate("你好Sw语言", 4)} ellipsis=${ellipsis("你好Sw语言", 4)}`);
    println(`remove_prefix=${remove_prefix("abc.def", "abc.")} remove_suffix=${remove_suffix("abc.def", ".def")}`);
    println(`is_upper=${is_upper("ABC")} is_lower=${is_lower("abc")} is_digit=${is_digit("123")} capitalize=${capitalize("sw")}`);
    println(`is_blank=${is_blank("  ")} strip_whitespace=[${strip_whitespace(" a  b ")}]`);
    println(`substring_between=${substring_between("<a>X</a>", "<a>", "</a>")}`);
    println(`substring_between_last=${substring_between_last("a#1 b#2", "#", " ")}`);
    println(`extract_between=${extract_between("[1] [2] [3]", "[", "]").length} before=${before("a|b|c", "|")} after=${after("a|b|c", "|")}`);
    println(`before_last=${before_last("a|b|c", "|")} after_last=${after_last("a|b|c", "|")}`);
    println(`char_code=${char_code("你好", 0)} replace_pairs=${replace_pairs("你好，世界", "你好", "Hello", "世界", "World")}`);

    // 中文函数名
    println(`反转文本=${反转文本("你好Sw")} 是否空白=${是否空白(" ")} 删全部空白=[${删全部空白(" a b ")}]`);
    println(`取文本中间=${取文本中间("<a>X</a>", "<a>", "</a>")} 取文本左边=${取文本左边("a|b", "|")} 取文本右边=${取文本右边("a|b", "|")}`);
    println(`是否包含=${是否包含("hello", "ell")} 开头为=${开头为("hello", "he")} 结尾为=${结尾为("hello", "lo")}`);
    println(`出现次数=${出现次数("ababa", "ab")} 首字母大写=${首字母大写("sw")} 删除前缀=${删除前缀("abc.def", "abc.")}`);
    return 0;
}
