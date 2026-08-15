import { println } from "std/io";
import { base32_encode, base32_decode, html_unescape, html_escape } from "std/encoding";
import { is_cjk, is_letter, is_digit, char_width } from "std/unicode";

function check(condition: bool, label: string): int {
    if (condition) {
        println(`[ok] ${label}`);
        return 1;
    }
    println(`[FAIL] ${label}`);
    return 0;
}

function main(): int {
    let passed = 1;

    // ---------- encoding：base32 ----------
    passed = passed & check(base32_encode("") == "", "base32 empty");
    passed = passed & check(base32_encode("f") == "MY======", "base32 1 byte");
    passed = passed & check(base32_encode("fo") == "MZXQ====", "base32 2 bytes");
    passed = passed & check(base32_encode("foo") == "MZXW6===", "base32 3 bytes");
    passed = passed & check(base32_encode("foob") == "MZXW6YQ=", "base32 4 bytes");
    passed = passed & check(base32_encode("fooba") == "MZXW6YTB", "base32 5 bytes");
    passed = passed & check(base32_encode("hello") == "NBSWY3DP", "base32 hello");

    passed = passed & check(base32_decode("") == "", "base32 decode empty");
    passed = passed & check(base32_decode("MY======") == "f", "base32 decode 1");
    passed = passed & check(base32_decode("MZXQ====") == "fo", "base32 decode 2");
    passed = passed & check(base32_decode("MZXW6===") == "foo", "base32 decode 3");
    passed = passed & check(base32_decode("MZXW6YQ=") == "foob", "base32 decode 4");
    passed = passed & check(base32_decode("MZXW6YTB") == "fooba", "base32 decode 5");
    passed = passed & check(base32_decode("NBSWY3DP") == "hello", "base32 decode hello");
    passed = passed & check(base32_decode(base32_encode("round trip 你好")) == "round trip 你好", "base32 round trip");

    // ---------- encoding：html_unescape ----------
    passed = passed & check(html_unescape("a&amp;b") == "a&b", "html_unescape amp");
    passed = passed & check(html_unescape("&lt;tag&gt;") == "<tag>", "html_unescape lt gt");
    passed = passed & check(html_unescape("&quot;q&quot;") == "\"q\"", "html_unescape quot");
    passed = passed & check(html_unescape("&#39;it&#39;s&#39;") == "'it's'", "html_unescape apos");
    passed = passed & check(html_unescape("&#65;&#x42;") == "AB", "html_unescape numeric");
    passed = passed & check(html_unescape("&#x4F60;&#x597D;") == "你好", "html_unescape CJK numeric");
    passed = passed & check(html_unescape("no entities here") == "no entities here", "html_unescape plain");
    passed = passed & check(html_unescape("a & b") == "a & b", "html_unescape bare amp");
    passed = passed & check(html_unescape(html_escape("a<b>&\"'")) == "a<b>&\"'", "html round trip");

    // ---------- unicode：is_cjk ----------
    passed = passed & check(is_cjk("你好"), "is_cjk hello");
    passed = passed & check(is_cjk("中文测试"), "is_cjk multi");
    passed = passed & check(is_cjk("你好a") == false, "is_cjk mixed false");
    passed = passed & check(is_cjk("hello") == false, "is_cjk ascii false");
    passed = passed & check(is_cjk("") == false, "is_cjk empty false");

    // ---------- unicode：is_letter ----------
    passed = passed & check(is_letter("abc"), "is_letter ascii");
    passed = passed & check(is_letter("HelloWorld"), "is_letter upper");
    passed = passed & check(is_letter("café"), "is_letter accent");
    passed = passed & check(is_letter("Привет"), "is_letter cyrillic");
    passed = passed & check(is_letter("a1") == false, "is_letter digit false");
    passed = passed & check(is_letter("你好"), "is_letter cjk true");
    passed = passed & check(is_letter("你a好"), "is_letter cjk mixed ascii");
    passed = passed & check(is_letter("你好1") == false, "is_letter cjk digit false");
    passed = passed & check(is_letter("") == false, "is_letter empty false");

    // ---------- unicode：is_digit ----------
    passed = passed & check(is_digit("123"), "is_digit ascii");
    passed = passed & check(is_digit("１２３"), "is_digit fullwidth");
    passed = passed & check(is_digit("١٢٣"), "is_digit arabic");
    passed = passed & check(is_digit("12a") == false, "is_digit mixed false");
    passed = passed & check(is_digit("") == false, "is_digit empty false");

    // ---------- unicode：char_width ----------
    passed = passed & check(char_width("") == 0, "char_width empty");
    passed = passed & check(char_width("ab") == 2, "char_width ascii");
    passed = passed & check(char_width("你好") == 4, "char_width cjk");
    passed = passed & check(char_width("你好ab") == 6, "char_width mixed");
    passed = passed & check(char_width("，。") == 4, "char_width fullwidth punct");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
