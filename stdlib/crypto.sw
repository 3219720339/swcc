// std/crypto - 安全随机字节与比较。摘要函数仍位于 std/hash。

import { bytes_base64url_encode, bytes_hex_encode } from "std/bytes";

/// 由操作系统安全随机源生成；长度 <= 0 返回空数组，系统随机源失败同样返回空数组。
export extern c function random_bytes(length: int): u8[];

/// 常量时间比较，适合 token/MAC；长度不同仍完整扫描较短输入后返回 false。
export function constant_time_equal(a: u8[], b: u8[]): bool {
    let different = a.length ^ b.length;
    const count = a.length < b.length ? a.length : b.length;
    let i = 0;
    while (i < count) {
        different = different | ((a[i] as int) ^ (b[i] as int));
        i++;
    }
    return different == 0;
}

/// URL 安全 token，不含填充；bytes_len <= 0 返回空串。
export function secure_token(bytes_len: int): string { return bytes_base64url_encode(random_bytes(bytes_len)); }

/// 随机字节的十六进制文本，长度为 2 * bytes_len。
export function secure_hex(bytes_len: int): string { return bytes_hex_encode(random_bytes(bytes_len)); }
