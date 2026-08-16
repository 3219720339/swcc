// std/audio - WAV/PCM 数据处理与播放器句柄。
// WAV 处理支持 PCM 8/16 位整数；Windows 播放器内置 WAV/MP3/FLAC 解码，
// 音频线程不回调 Sw 代码。Linux/macOS 播放设备后端仍在打包中，数据 API 可用。

import { read_file_bytes, write_file_bytes } from "std/fs";
import { bytes_read_u16_le, bytes_read_u32_le } from "std/bytes";

export const AUDIO_STOPPED = 0;
export const AUDIO_PLAYING = 1;
export const AUDIO_PAUSED = 2;
export const AUDIO_ENDED = 3;

export struct WavInfo {
    valid: bool;
    format: int;
    channels: int;
    sample_rate: int;
    bits_per_sample: int;
    data_offset: int;
    data_size: int;
    duration_ms: int;
}

function tag_at(data: u8[], at: int, a: int, b: int, c: int, d: int): bool {
    return at >= 0 && at + 4 <= data.length && data[at] == a as u8 && data[at + 1] == b as u8 && data[at + 2] == c as u8 && data[at + 3] == d as u8;
}

/// 解析 RIFF/WAVE；不支持或损坏文件返回 valid=false。
export function wav_info_bytes(data: u8[]): WavInfo {
    if (data.length < 12 || !tag_at(data, 0, 82, 73, 70, 70) || !tag_at(data, 8, 87, 65, 86, 69)) {
        return { valid: false, format: 0, channels: 0, sample_rate: 0, bits_per_sample: 0, data_offset: 0, data_size: 0, duration_ms: 0 };
    }
    let format = 0;
    let channels = 0;
    let rate = 0;
    let bits = 0;
    let data_at = 0;
    let data_size = 0;
    let at = 12;
    while (at + 8 <= data.length) {
        const size = bytes_read_u32_le(data, at + 4);
        if (size < 0 || at + 8 + size > data.length) { break; }
        if (tag_at(data, at, 102, 109, 116, 32) && size >= 16) {
            format = bytes_read_u16_le(data, at + 8);
            channels = bytes_read_u16_le(data, at + 10);
            rate = bytes_read_u32_le(data, at + 12);
            bits = bytes_read_u16_le(data, at + 22);
        }
        if (tag_at(data, at, 100, 97, 116, 97)) { data_at = at + 8; data_size = size; break; }
        at = at + 8 + size + (size % 2);
    }
    const bytes_per_sec = channels * rate * (bits / 8);
    const valid = format == 1 && channels > 0 && rate > 0 && (bits == 8 || bits == 16) && data_size >= 0 && bytes_per_sec > 0;
    return { valid, format, channels, sample_rate: rate, bits_per_sample: bits, data_offset: data_at, data_size, duration_ms: valid ? data_size * 1000 / bytes_per_sec : 0 };
}

export function wav_info(path: string): WavInfo { return wav_info_bytes(read_file_bytes(path)); }
export function wav_duration_ms(path: string): int { return wav_info(path).duration_ms; }

function put_u32_le(data: u8[], at: int, value: int): void {
    data[at] = (value & 255) as u8; data[at + 1] = ((value >> 8) & 255) as u8;
    data[at + 2] = ((value >> 16) & 255) as u8; data[at + 3] = ((value >> 24) & 255) as u8;
}

/// 线性 PCM 音量，percent 范围 0..200；仅修改 data chunk，返回空数组表示不支持。
export function wav_gain_bytes(data: u8[], percent: int): u8[] {
    const info = wav_info_bytes(data); if (!info.valid) { return []; }
    const out: u8[] = []; let copy = 0; while (copy < data.length) { out.push(data[copy]); copy++; }
    const gain = percent < 0 ? 0 : (percent > 200 ? 200 : percent);
    let i = info.data_offset;
    if (info.bits_per_sample == 8) {
        while (i < info.data_offset + info.data_size) { let v = ((out[i] as int) - 128) * gain / 100 + 128; out[i] = (v < 0 ? 0 : (v > 255 ? 255 : v)) as u8; i++; }
    } else {
        while (i + 1 < info.data_offset + info.data_size) { let v = (out[i] as int) | ((out[i + 1] as int) << 8); if (v >= 32768) { v -= 65536; } v = v * gain / 100; v = v < -32768 ? -32768 : (v > 32767 ? 32767 : v); if (v < 0) { v += 65536; } out[i] = (v & 255) as u8; out[i + 1] = ((v >> 8) & 255) as u8; i += 2; }
    }
    return out;
}

/// 线性重采样变速。speed_percent=200 表示两倍速，输出时长减半。
export function wav_speed_bytes(data: u8[], speed_percent: int): u8[] {
    const info = wav_info_bytes(data); if (!info.valid || speed_percent < 25 || speed_percent > 400) { return []; }
    const frame = info.channels * (info.bits_per_sample / 8); const frames = info.data_size / frame;
    const out_frames = frames * 100 / speed_percent; const out_size = out_frames * frame;
    const out: u8[] = [];
    let h = 0; while (h < info.data_offset) { out.push(data[h]); h++; }
    let n = 0; while (n < out_frames) {
        const source_frame = n * speed_percent / 100; const fraction = (n * speed_percent) % 100;
        const next_frame = source_frame + 1 < frames ? source_frame + 1 : source_frame;
        let b = 0;
        while (b < frame) {
            const src = info.data_offset + source_frame * frame + b; const next = info.data_offset + next_frame * frame + b;
            if (info.bits_per_sample == 8) {
                const a = (data[src] as int) - 128; const z = (data[next] as int) - 128;
                out.push((a + (z - a) * fraction / 100 + 128) as u8);
                b++;
            } else {
                const a_raw = (data[src] as int) | ((data[src + 1] as int) << 8); const z_raw = (data[next] as int) | ((data[next + 1] as int) << 8);
                const a = a_raw >= 32768 ? a_raw - 65536 : a_raw; const z = z_raw >= 32768 ? z_raw - 65536 : z_raw;
                const value = a + (z - a) * fraction / 100; const encoded = value < 0 ? value + 65536 : value;
                out.push((encoded & 255) as u8); out.push(((encoded >> 8) & 255) as u8); b += 2;
            }
        }
        n++;
    }
    put_u32_le(out, 4, out.length - 8); put_u32_le(out, info.data_offset - 4, out_size); return out;
}

function pcm_sample(data: u8[], at: int, bits: int): int {
    if (bits == 8) { return (data[at] as int) - 128; }
    let value = (data[at] as int) | ((data[at + 1] as int) << 8);
    return value >= 32768 ? value - 65536 : value;
}

function pcm_write(data: u8[], at: int, bits: int, value: int): void {
    if (bits == 8) { data[at] = (value + 128 < 0 ? 0 : (value + 128 > 255 ? 255 : value + 128)) as u8; return; }
    const clipped = value < -32768 ? -32768 : (value > 32767 ? 32767 : value);
    const encoded = clipped < 0 ? clipped + 65536 : clipped;
    data[at] = (encoded & 255) as u8; data[at + 1] = ((encoded >> 8) & 255) as u8;
}

/// 对 PCM 数据做淡入；duration_ms 超过文件时长时按整段淡入。
export function wav_fade_in_bytes(data: u8[], duration_ms: int): u8[] {
    const info = wav_info_bytes(data); if (!info.valid || duration_ms <= 0) { return data; }
    const out: u8[] = []; let copy = 0; while (copy < data.length) { out.push(data[copy]); copy++; }
    const frame_bytes = info.channels * (info.bits_per_sample / 8);
    const fade_frames = info.sample_rate * duration_ms / 1000;
    const frames = info.data_size / frame_bytes;
    let frame = 0; while (frame < frames && frame < fade_frames) {
        const gain = frame * 100 / (fade_frames <= 1 ? 1 : fade_frames - 1);
        let channel = 0; while (channel < frame_bytes) {
            const at = info.data_offset + frame * frame_bytes + channel;
            if (info.bits_per_sample == 8) { out[at] = (pcm_sample(out, at, 8) * gain / 100 + 128) as u8; }
            else if (channel % 2 == 0) { pcm_write(out, at, 16, pcm_sample(out, at, 16) * gain / 100); }
            channel = info.bits_per_sample == 16 ? channel + 2 : channel + 1;
        }
        frame++;
    }
    return out;
}

/// 对 PCM 数据做淡出；duration_ms 超过文件时长时按整段淡出。
export function wav_fade_out_bytes(data: u8[], duration_ms: int): u8[] {
    const info = wav_info_bytes(data); if (!info.valid || duration_ms <= 0) { return data; }
    const out: u8[] = []; let copy = 0; while (copy < data.length) { out.push(data[copy]); copy++; }
    const frame_bytes = info.channels * (info.bits_per_sample / 8);
    const fade_frames = info.sample_rate * duration_ms / 1000; const frames = info.data_size / frame_bytes;
    let frame = frames - fade_frames; if (frame < 0) { frame = 0; }
    while (frame < frames) {
        const gain = (frames - 1 - frame) * 100 / (frames - 1 - (frames - fade_frames) <= 0 ? 1 : frames - 1 - (frames - fade_frames));
        let channel = 0; while (channel < frame_bytes) {
            const at = info.data_offset + frame * frame_bytes + channel;
            if (info.bits_per_sample == 8) { out[at] = (pcm_sample(out, at, 8) * gain / 100 + 128) as u8; }
            else if (channel % 2 == 0) { pcm_write(out, at, 16, pcm_sample(out, at, 16) * gain / 100); }
            channel = info.bits_per_sample == 16 ? channel + 2 : channel + 1;
        }
        frame++;
    }
    return out;
}

/// 将第二个同格式 PCM WAV 混入第一个文件，overlay_percent 范围 0..200。
export function wav_mix_bytes(base: u8[], overlay: u8[], overlay_percent: int): u8[] {
    const a = wav_info_bytes(base); const b = wav_info_bytes(overlay);
    if (!a.valid || !b.valid || a.channels != b.channels || a.sample_rate != b.sample_rate || a.bits_per_sample != b.bits_per_sample) { return []; }
    const out: u8[] = []; let i = 0; while (i < base.length) { out.push(base[i]); i++; }
    const gain = overlay_percent < 0 ? 0 : (overlay_percent > 200 ? 200 : overlay_percent);
    const frame_bytes = a.channels * (a.bits_per_sample / 8); const frames = a.data_size / frame_bytes; const overlay_frames = b.data_size / frame_bytes;
    let frame = 0; while (frame < frames && frame < overlay_frames) {
        let channel = 0; while (channel < frame_bytes) {
            const at = a.data_offset + frame * frame_bytes + channel; const bt = b.data_offset + frame * frame_bytes + channel;
            if (a.bits_per_sample == 8) { pcm_write(out, at, 8, pcm_sample(out, at, 8) + pcm_sample(overlay, bt, 8) * gain / 100); }
            else if (channel % 2 == 0) { pcm_write(out, at, 16, pcm_sample(out, at, 16) + pcm_sample(overlay, bt, 16) * gain / 100); }
            channel = a.bits_per_sample == 16 ? channel + 2 : channel + 1;
        }
        frame++;
    }
    return out;
}

/// 生成 0..100 的每桶峰值，适合绘制波形预览。
export function wav_waveform_peaks(data: u8[], buckets: int): int[] {
    const info = wav_info_bytes(data); const peaks: int[] = [];
    if (!info.valid || buckets <= 0) { return peaks; }
    const frame_bytes = info.channels * (info.bits_per_sample / 8); const frames = info.data_size / frame_bytes;
    let bucket = 0; while (bucket < buckets) {
        const start = bucket * frames / buckets; const end = (bucket + 1) * frames / buckets; let peak = 0; let frame = start;
        while (frame < end && frame < frames) { let channel = 0; while (channel < frame_bytes) { const at = info.data_offset + frame * frame_bytes + channel; const value = pcm_sample(data, at, info.bits_per_sample); const magnitude = value < 0 ? -value : value; if (magnitude > peak) { peak = magnitude; } channel = info.bits_per_sample == 16 ? channel + 2 : channel + 1; } frame++; }
        peaks.push(info.bits_per_sample == 8 ? peak * 100 / 128 : peak * 100 / 32768); bucket++;
    }
    return peaks;
}

export function wav_gain_file(source: string, destination: string, percent: int): int { return write_file_bytes(destination, wav_gain_bytes(read_file_bytes(source), percent)); }
export function wav_speed_file(source: string, destination: string, speed_percent: int): int { return write_file_bytes(destination, wav_speed_bytes(read_file_bytes(source), speed_percent)); }

export extern c function audio_open(path: string): int;
export extern c function audio_play(handle: int): int;
export extern c function audio_pause(handle: int): int;
export extern c function audio_resume(handle: int): int;
export extern c function audio_stop(handle: int): int;
export extern c function audio_close(handle: int): int;
export extern c function audio_state(handle: int): int;
export extern c function audio_position_ms(handle: int): int;
export extern c function audio_duration_ms(handle: int): int;
export extern c function audio_seek(handle: int, position_ms: int): int;
export extern c function audio_set_volume(handle: int, percent: int): int;
export extern c function audio_set_speed(handle: int, speed_percent: int): int;
