import { println } from "std/io";
import {
    utf8_len,
    utf8_char_at,
    utf8_substring,
    utf8_byte_len,
    utf8_index_to_byte,
    utf8_byte_to_index,
    utf8_is_printable,
} from "std/unicode";

function main(): int {
    const s = "你好Sw";
    println(`text=${s} len=${utf8_len(s)} byte_len=${utf8_byte_len(s)}`);
    println(`char_at(0)=${utf8_char_at(s, 0)} char_at(1)=${utf8_char_at(s, 1)} char_at(3)=${utf8_char_at(s, 3)}`);
    println(`substring(0,2)=${utf8_substring(s, 0, 2)} substring(2,2)=${utf8_substring(s, 2, 2)}`);
    println(`index_to_byte(0)=${utf8_index_to_byte(s, 0)} index_to_byte(1)=${utf8_index_to_byte(s, 1)} index_to_byte(2)=${utf8_index_to_byte(s, 2)}`);
    println(`byte_to_index(3)=${utf8_byte_to_index(s, 3)} byte_to_index(6)=${utf8_byte_to_index(s, 6)}`);
    println(`is_printable=${utf8_is_printable("你好，Sw")} is_printable_ctrl=${utf8_is_printable("a\u{1}b")}`);
    println(`length=${s.length} first_char=${s[0]}`);
    return 0;
}
