// std/bytes - 二进制数据辅助。所有读取越界返回 -1，切片会钳制到有效范围。

import { base64_encode, base64_decode, base64url_encode, base64url_decode } from "std/encoding";
import { from_utf8_bytes, to_utf8_bytes, char_code, int_to_hex } from "std/string";

export function bytes_concat(a: u8[], b: u8[]): u8[] {
    const out: u8[] = [];
    for (const value of a) { out.push(value); }
    for (const value of b) { out.push(value); }
    return out;
}

export function bytes_slice(bytes: u8[], start: int, length: int): u8[] {
    const out: u8[] = [];
    let at = start < 0 ? 0 : start;
    const end = length < 0 ? at : at + length;
    while (at < bytes.length && at < end) { out.push(bytes[at]); at++; }
    return out;
}

export function bytes_equal(a: u8[], b: u8[]): bool {
    if (a.length != b.length) { return false; }
    let different = 0;
    let i = 0;
    while (i < a.length) { different = different | ((a[i] as int) ^ (b[i] as int)); i++; }
    return different == 0;
}

export function bytes_hex_encode(bytes: u8[]): string {
    let out = "";
    for (const value of bytes) {
        const part = int_to_hex(value as int);
        out = out + ((value as int) < 16 ? "0" : "") + part;
    }
    return out;
}

function hex_value(code: int): int {
    if (code >= 48 && code <= 57) { return code - 48; }
    if (code >= 65 && code <= 70) { return code - 65 + 10; }
    if (code >= 97 && code <= 102) { return code - 97 + 10; }
    return -1;
}

/// 非法十六进制或奇数长度返回空数组。
export function bytes_hex_decode(text: string): u8[] {
    const out: u8[] = [];
    if (text.length % 2 != 0) { return out; }
    let i = 0;
    while (i < text.length) {
        const hi = hex_value(char_code(text, i));
        const lo = hex_value(char_code(text, i + 1));
        if (hi < 0 || lo < 0) { return []; }
        out.push((hi * 16 + lo) as u8);
        i += 2;
    }
    return out;
}

export function bytes_base64_encode(bytes: u8[]): string { return base64_encode(from_utf8_bytes(bytes)); }
export function bytes_base64_decode(text: string): u8[] { return to_utf8_bytes(base64_decode(text)); }
export function bytes_base64url_encode(bytes: u8[]): string { return base64url_encode(from_utf8_bytes(bytes)); }
export function bytes_base64url_decode(text: string): u8[] { return to_utf8_bytes(base64url_decode(text)); }

export function bytes_read_u16_le(bytes: u8[], offset: int): int {
    if (offset < 0 || offset + 2 > bytes.length) { return -1; }
    return (bytes[offset] as int) | ((bytes[offset + 1] as int) << 8);
}
export function bytes_read_u16_be(bytes: u8[], offset: int): int {
    if (offset < 0 || offset + 2 > bytes.length) { return -1; }
    return ((bytes[offset] as int) << 8) | (bytes[offset + 1] as int);
}
export function bytes_read_u32_le(bytes: u8[], offset: int): int {
    if (offset < 0 || offset + 4 > bytes.length) { return -1; }
    return (bytes[offset] as int) | ((bytes[offset + 1] as int) << 8) | ((bytes[offset + 2] as int) << 16) | ((bytes[offset + 3] as int) << 24);
}
export function bytes_read_u32_be(bytes: u8[], offset: int): int {
    if (offset < 0 || offset + 4 > bytes.length) { return -1; }
    return ((bytes[offset] as int) << 24) | ((bytes[offset + 1] as int) << 16) | ((bytes[offset + 2] as int) << 8) | (bytes[offset + 3] as int);
}
export function bytes_u16_le(value: int): u8[] { return [(value & 255) as u8, ((value >> 8) & 255) as u8]; }
export function bytes_u16_be(value: int): u8[] { return [((value >> 8) & 255) as u8, (value & 255) as u8]; }
export function bytes_u32_le(value: int): u8[] { return [(value & 255) as u8, ((value >> 8) & 255) as u8, ((value >> 16) & 255) as u8, ((value >> 24) & 255) as u8]; }
export function bytes_u32_be(value: int): u8[] { return [((value >> 24) & 255) as u8, ((value >> 16) & 255) as u8, ((value >> 8) & 255) as u8, (value & 255) as u8]; }
