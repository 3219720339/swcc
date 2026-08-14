// ===========================================================================
// std/hash —— 字符串哈希
//
// 用法：
//   import { fnv1a_64, fnv1a_64_seed, djb2 } from "std/hash";
//   const h1 = fnv1a_64("hello");         // 64 位 FNV-1a
//   const h2 = fnv1a_64_seed("hello", 0); // 带自定义种子
//   const h3 = djb2("hello");             // DJB2
//
// 说明：
//   - 返回值为 int（64 位有符号；FNV-1a 的 64 位偏移基数为通常内建值）。
//   - 用作 map 键/去重/指纹时注意只比较同类哈希值。
// ===========================================================================

/// FNV-1a 64 位哈希（标准 64 位素数+偏移基数）。
export extern c function fnv1a_64(text: string): int;

/// FNV-1a 64 位哈希，带自定义初始种子。
export extern c function fnv1a_64_seed(text: string, seed: int): int;

/// DJB2 字符串哈希（初始 5381）。
export extern c function djb2(text: string): int;

/// MD5 哈希（32 位十六进制小写）。示例：md5("hello") == "5d41402abc4b2a76b9719d911017c592"。
export extern c function md5(text: string): string;

/// 文件 MD5（按字节流计算，适合校验文件）；文件不存在/读取失败返回空串。
export extern c function md5_file(path: string): string;

/// SHA-256 哈希（64 位十六进制小写）。示例：sha256("hello") 为标准 64 位摘要。
export extern c function sha256(text: string): string;

/// 文件 SHA-256（按字节流计算）；文件不存在/读取失败返回空串。
export extern c function sha256_file(path: string): string;

// ---------------------------------------------------------------------------
// 中文函数名（转发到英文实现，火山风格命名）
// ---------------------------------------------------------------------------

export function 取MD5(text: string): string {
    return md5(text);
}

export function 取MD5文件(path: string): string {
    return md5_file(path);
}

export function 取SHA256(text: string): string {
    return sha256(text);
}

export function 取SHA256文件(path: string): string {
    return sha256_file(path);
}

/// CRC-32（IEEE 802.3，无表逐位）；返回 32 位无符号位模式。
/// 示例：crc32("hello") == 0x3610A686。
export extern c function crc32(text: string): int;

/// 文件 CRC-32（按字节流计算）；文件不存在/读取失败返回 0。
export extern c function crc32_file(path: string): int;

/// CRC-16（CRC-16/IBM，反射多项式 0xA001）；返回 16 位无符号位模式。
export extern c function crc16(text: string): int;

/// SHA-1 哈希（40 位十六进制小写）。示例：sha1("abc") 为标准 40 位摘要。
export extern c function sha1(text: string): string;

/// 文件 SHA-1（按字节流计算）；文件不存在/读取失败返回空串。
export extern c function sha1_file(path: string): string;

/// HMAC-SHA256（密钥 + 消息，64 位十六进制小写）。key 超 64 字节先哈希。
export extern c function hmac_sha256(key: string, text: string): string;

export function 取CRC32(text: string): int {
    return crc32(text);
}

export function 取CRC32文件(path: string): int {
    return crc32_file(path);
}

export function 取CRC16(text: string): int {
    return crc16(text);
}

export function 取SHA1(text: string): string {
    return sha1(text);
}

export function 取SHA1文件(path: string): string {
    return sha1_file(path);
}

export function 取HMACSHA256(key: string, text: string): string {
    return hmac_sha256(key, text);
}
