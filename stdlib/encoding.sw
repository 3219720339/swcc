// ===========================================================================
// std/encoding —— 文本编码
//
// 用法：
//   import { base64_encode, base64_decode, hex_encode, hex_decode,
//            url_encode, url_decode } from "std/encoding";
//   const b64 = base64_encode("hello");   // "aGVsbG8="
//   const hex = hex_encode("hello");      // "68656c6c6f"
//   const q = url_encode("a b&c");        // "a%20b%26c"
//
// 说明：输入按 UTF-8 原始字节处理；解码函数对非法输入采取宽容策略
// （遇到非法字符即停止），结果按原始字节返回。
// ===========================================================================

/// Base64 编码（标准字母表 + 填充 =）。
export extern c function base64_encode(text: string): string;

/// Base64 解码，返回原始字节（可能不是合法 UTF-8）。
export extern c function base64_decode(text: string): string;

/// 十六进制编码（小写字母）。
export extern c function hex_encode(text: string): string;

/// 十六进制解码，返回原始字节；输入非偶数长度或含非法字符时截断/返回空串。
export extern c function hex_decode(text: string): string;

/// URL 百分号编码（RFC 3986：保留字母数字与 -_.~，其余 %XX）。
export extern c function url_encode(text: string): string;

/// URL 百分号解码；%XX 非法时按字面保留。
export extern c function url_decode(text: string): string;

/// Base64URL 编码（- _ 字母表，无填充）。
export extern c function base64url_encode(text: string): string;

/// Base64URL 解码（容忍标准 base64 与填充）。
export extern c function base64url_decode(text: string): string;

/// HTML 转义：& < > " ' -> &amp; &lt; &gt; &quot; &#39;。
export extern c function html_escape(text: string): string;
