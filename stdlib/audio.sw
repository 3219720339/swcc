// std/audio - WAV/PCM 数据处理与播放器句柄。
// WAV 处理支持 PCM 8/16 位整数；Windows 播放器内置 WAV/MP3/FLAC 解码，
// 音频线程不回调 Sw 代码。Linux/macOS 播放设备后端仍在打包中，数据 API 可用。

import { read_file_bytes, write_file_bytes } from "std/fs";
import { bytes_read_u16_le, bytes_read_u32_le } from "std/bytes";
import { from_utf8_bytes, format_int, index_of, substring, to_upper } from "std/string";

/// 音频文件元数据；字段在无法解析时保持默认值。
export struct AudioMetadata {
    valid: bool;
    format: string;
    channels: int;
    sample_rate: int;
    bits_per_sample: int;
    bitrate_kbps: int;
    duration_ms: int;
    title: string;
    artist: string;
    album: string;
}

/// 播放器状态：已停止。
export const AUDIO_STOPPED = 0;
/// 播放器状态：播放中。
export const AUDIO_PLAYING = 1;
/// 播放器状态：已暂停。
export const AUDIO_PAUSED = 2;
/// 播放器状态：自然播放结束。
export const AUDIO_ENDED = 3;
/// 播放器状态：发生错误。
export const AUDIO_FAILED = 4;

/// WAV PCM 头和数据块信息。
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
/// 从内存字节解析 RIFF/WAVE 信息。
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

/// 从文件解析 WAV 信息。
export function wav_info(path: string): WavInfo { return wav_info_bytes(read_file_bytes(path)); }
/// 返回 WAV 文件时长（毫秒）；文件无效时返回 0。
export function wav_duration_ms(path: string): int { return wav_info(path).duration_ms; }

/// 将毫秒时长格式化为 HH:MM:SS；不足一小时则返回 MM:SS。
export function audio_format_duration_ms(duration_ms: int): string {
    if (duration_ms < 0) { return "--:--"; }
    const total = duration_ms / 1000; const seconds = total % 60; const minutes = (total / 60) % 60; const hours = total / 3600;
    if (hours > 0) { return format_int(hours, 2, 1) + ":" + format_int(minutes, 2, 1) + ":" + format_int(seconds, 2, 1); }
    return format_int(minutes, 2, 1) + ":" + format_int(seconds, 2, 1);
}

function metadata_empty(): AudioMetadata {
    return { valid: false, format: "", channels: 0, sample_rate: 0, bits_per_sample: 0,
        bitrate_kbps: 0, duration_ms: 0, title: "", artist: "", album: "" };
}

function metadata_text(data: u8[], at: int, size: int): string {
    if (at < 0 || size <= 0 || at + size > data.length) { return ""; }
    const bytes: u8[] = []; let i = 0;
    while (i < size && data[at + i] != 0) { bytes.push(data[at + i]); i++; }
    return from_utf8_bytes(bytes);
}

function utf8_push_codepoint(out: u8[], codepoint: int): void {
    if (codepoint < 128) { out.push(codepoint as u8); }
    else if (codepoint < 2048) { out.push((192 | (codepoint / 64)) as u8); out.push((128 | (codepoint & 63)) as u8); }
    else if (codepoint < 65536) { out.push((224 | (codepoint / 4096)) as u8); out.push((128 | ((codepoint / 64) & 63)) as u8); out.push((128 | (codepoint & 63)) as u8); }
}

function id3_text(data: u8[], at: int, size: int): string {
    if (size <= 1) { return ""; }
    const encoding = data[at] as int; const bytes: u8[] = [];
    if (encoding == 0 || encoding == 3) { let i = 1; while (i < size && data[at + i] != 0) { bytes.push(data[at + i]); i++; } return from_utf8_bytes(bytes); }
    const little = encoding == 1 && at + 2 < data.length && data[at + 1] == 255 && data[at + 2] == 254;
    let i = little ? 3 : (encoding == 1 && at + 2 < data.length && data[at + 1] == 254 && data[at + 2] == 255 ? 3 : 1);
    while (i + 1 < size) {
        const first = data[at + i] as int; const second = data[at + i + 1] as int; const codepoint = little ? first | (second << 8) : (first << 8) | second;
        if (codepoint == 0) { break; }
        utf8_push_codepoint(bytes, codepoint); i += 2;
    }
    return from_utf8_bytes(bytes);
}

/// 读取 WAV/MP3/FLAC 的基础元数据；未支持或损坏数据返回 valid=false。
/// 从内存字节读取 WAV/MP3/FLAC 元数据。
export function audio_metadata_bytes(data: u8[]): AudioMetadata {
    let result = metadata_empty();
    const wav = wav_info_bytes(data);
    if (wav.valid) {
        result.valid = true; result.format = "wav"; result.channels = wav.channels;
        result.sample_rate = wav.sample_rate; result.bits_per_sample = wav.bits_per_sample;
        result.duration_ms = wav.duration_ms; return result;
    }
    if (data.length >= 4 && tag_at(data, 0, 102, 76, 97, 67)) {
        let at = 4;
        while (at + 4 <= data.length) {
            const block_type = (data[at] as int) & 127; const size = ((data[at + 1] as int) << 16) | ((data[at + 2] as int) << 8) | (data[at + 3] as int);
            if (at + 4 + size > data.length) { break; }
            if (block_type == 0 && size >= 18) {
                const rate = ((data[at + 14] as int) << 12) | ((data[at + 15] as int) << 4) | ((data[at + 16] as int) >> 4);
                const channels = (((data[at + 16] as int) >> 1) & 7) + 1;
                const bits = (((data[at + 16] as int) & 1) << 4) | ((data[at + 17] as int) >> 4) + 1;
                const total = ((data[at + 17] as int) & 15) * 16777216 + (data[at + 18] as int) * 65536 + (data[at + 19] as int) * 256 + (data[at + 20] as int);
                result.valid = true; result.format = "flac"; result.channels = channels; result.sample_rate = rate;
                result.bits_per_sample = bits; result.duration_ms = rate > 0 ? total * 1000 / rate : 0;
            }
            if (block_type == 4 && size >= 8) {
                let cursor = at + 8; const end = at + 4 + size;
                const vendor_len = bytes_read_u32_le(data, cursor); cursor += 4 + vendor_len;
                if (cursor + 4 <= end) {
                    const count = bytes_read_u32_le(data, cursor); cursor += 4; let item = 0;
                    while (item < count && cursor + 4 <= end) {
                        const item_len = bytes_read_u32_le(data, cursor); cursor += 4;
                        if (item_len < 0 || cursor + item_len > end) { break; }
                        const text = metadata_text(data, cursor, item_len); const equals = index_of(text, "=");
                        if (equals > 0) {
                            const key = to_upper(substring(text, 0, equals)); const value = substring(text, equals + 1, item_len - equals - 1);
                            if (key == "TITLE") { result.title = value; }
                            if (key == "ARTIST") { result.artist = value; }
                            if (key == "ALBUM") { result.album = value; }
                        }
                        cursor += item_len; item++;
                    }
                }
            }
            const last_block = (data[at] as int) >= 128; at = at + 4 + size; if (last_block) { break; }
        }
        return result;
    }
    if (data.length >= 4 && tag_at(data, 0, 73, 68, 51, 3)) {
        result.valid = true; result.format = "mp3"; let offset = 10;
        const tag_size = ((data[6] as int) & 127) * 2097152 + ((data[7] as int) & 127) * 16384 + ((data[8] as int) & 127) * 128 + ((data[9] as int) & 127);
        offset = 10 + tag_size;
        let frame = offset;
        while (frame + 4 <= data.length && frame < offset + 4096) {
            if (data[frame] == 255 && (data[frame + 1] as int) >= 224) {
                const version = ((data[frame + 1] as int) >> 3) & 3; const layer = ((data[frame + 1] as int) >> 1) & 3;
                const bitrate_index = ((data[frame + 2] as int) >> 4) & 15; const rate_index = ((data[frame + 2] as int) >> 2) & 3;
                const rates: int[] = [44100, 48000, 32000]; const bitrates: int[] = [0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 0];
                if (layer == 1 && rate_index < 3 && bitrate_index < 16) {
                    result.sample_rate = version == 3 ? rates[rate_index] : rates[rate_index] / 2;
                    result.bitrate_kbps = bitrates[bitrate_index]; result.channels = ((data[frame + 3] as int) >> 6) == 3 ? 1 : 2;
                    result.duration_ms = result.bitrate_kbps > 0 ? (data.length - offset) * 8 * 1000 / (result.bitrate_kbps * 1000) : 0;
                }
                break;
            }
            frame++;
        }
        let at = 10; while (at + 10 <= 10 + tag_size && at + 10 <= data.length) {
            const frame_id = metadata_text(data, at, 4); const size = ((data[at + 4] as int) << 24) | ((data[at + 5] as int) << 16) | ((data[at + 6] as int) << 8) | (data[at + 7] as int);
            if (size <= 0 || at + 10 + size > data.length) { break; }
            if (frame_id == "TIT2") { result.title = id3_text(data, at + 10, size); }
            if (frame_id == "TPE1") { result.artist = id3_text(data, at + 10, size); }
            if (frame_id == "TALB") { result.album = id3_text(data, at + 10, size); }
            at = at + 10 + size;
        }
        return result;
    }
    return result;
}

/// 读取 WAV/MP3/FLAC 文件的元数据。
export function audio_metadata(path: string): AudioMetadata { return audio_metadata_bytes(read_file_bytes(path)); }

function put_u32_le(data: u8[], at: int, value: int): void {
    data[at] = (value & 255) as u8; data[at + 1] = ((value >> 8) & 255) as u8;
    data[at + 2] = ((value >> 16) & 255) as u8; data[at + 3] = ((value >> 24) & 255) as u8;
}

/// 线性 PCM 音量，percent 范围 0..200；仅修改 data chunk，返回空数组表示不支持。
/// 对内存中的 WAV PCM 应用音量增益。
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
/// 对内存中的 WAV PCM 做线性重采样变速。
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
/// 对 WAV PCM 应用渐入效果。
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
/// 对 WAV PCM 应用渐出效果。
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
/// 将同格式 WAV PCM 叠加混音并返回新字节数组。
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
/// 生成 0..100 的分桶波形峰值。
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

/// 将 WAV 增益效果应用并写入目标文件。
export function wav_gain_file(source: string, destination: string, percent: int): int { return write_file_bytes(destination, wav_gain_bytes(read_file_bytes(source), percent)); }
/// 将 WAV 线性变速效果应用并写入目标文件。
export function wav_speed_file(source: string, destination: string, speed_percent: int): int { return write_file_bytes(destination, wav_speed_bytes(read_file_bytes(source), speed_percent)); }

/// 打开音频文件并返回播放器句柄。
export extern c function audio_open(path: string): int;
/// 开始或重新开始播放。
export extern c function audio_play(handle: int): int;
/// 暂停播放并保留当前位置。
export extern c function audio_pause(handle: int): int;
/// 从暂停位置继续播放。
export extern c function audio_resume(handle: int): int;
/// 停止播放并将位置复位到 0。
export extern c function audio_stop(handle: int): int;
/// 关闭句柄并释放解码器、设备和队列资源。
export extern c function audio_close(handle: int): int;
/// 读取播放器状态常量。
export extern c function audio_state(handle: int): int;
/// 读取当前播放位置（毫秒）。
export extern c function audio_position_ms(handle: int): int;
/// 读取当前音频总时长（毫秒）。
export extern c function audio_duration_ms(handle: int): int;
/// 读取当前进度百分比（0..100）。
export extern c function audio_progress_percent(handle: int): int;
/// 请求跳转到指定毫秒位置。
export extern c function audio_seek(handle: int, position_ms: int): int;
/// 设置音量（0..200%，100 为原始音量）。
export extern c function audio_set_volume(handle: int, percent: int): int;
/// 设置播放速度（25..400%，100 为正常速度）。
export extern c function audio_set_speed(handle: int, speed_percent: int): int;
/// 读取最近一次错误码。
export extern c function audio_last_error(handle: int): int;
/// 返回可用播放设备数量。
export extern c function audio_device_count(): int;
/// 返回指定播放设备名称；索引无效时返回空字符串。
export extern c function audio_device_name(index: int): string;
/// 设置后续打开句柄使用的默认播放设备索引。
export extern c function audio_set_default_device(index: int): int;
/// 运行中切换该句柄的输出设备。
export extern c function audio_set_device(handle: int, index: int): int;
/// 将待播放文件加入队列，播放结束后自动切换。
export extern c function audio_queue(handle: int, path: string): int;
/// 返回待播放队列长度，不包含当前曲目。
export extern c function audio_queue_count(handle: int): int;
/// 清空待播放队列，不影响当前曲目。
export extern c function audio_queue_clear(handle: int): int;
/// 移除待播放队列中指定索引的曲目。
export extern c function audio_queue_remove(handle: int, index: int): int;
/// 将曲目插入待播放队列的指定索引。
export extern c function audio_queue_insert(handle: int, index: int, path: string): int;
/// 立即切换到下一首；队列为空时返回 -1。
export extern c function audio_next_track(handle: int): int;
/// 返回上一首；没有历史曲目时返回 -1。
export extern c function audio_previous_track(handle: int): int;
/// 设置循环模式：AUDIO_REPEAT_OFF、AUDIO_REPEAT_ALL 或 AUDIO_REPEAT_ONE。
export extern c function audio_set_repeat_mode(handle: int, mode: int): int;
/// 启用或关闭下一首随机选择。
export extern c function audio_set_shuffle(handle: int, enabled: int): int;
/// 设置自动切歌交叉淡化时长（0..30000 毫秒）。
export extern c function audio_set_crossfade_ms(handle: int, duration_ms: int): int;
/// 轮询一个播放器事件；无事件返回 0。
export extern c function audio_event_poll(handle: int): int;
/// 返回最近一次取出的事件发生位置（毫秒）。
export extern c function audio_event_position_ms(handle: int): int;

/// 事件队列：当前没有事件。
export const AUDIO_EVENT_NONE = 0;
/// 事件队列：当前曲目播放结束。
export const AUDIO_EVENT_ENDED = 1;
/// 事件队列：队列切换或解码发生错误。
export const AUDIO_EVENT_ERROR = 2;
/// 事件队列：输出设备丢失或无法启动。
export const AUDIO_EVENT_DEVICE_LOST = 3;
/// 事件队列：当前曲目已切换。
export const AUDIO_EVENT_TRACK_CHANGED = 4;

/// 不循环播放。
export const AUDIO_REPEAT_OFF = 0;
/// 队列循环播放。
export const AUDIO_REPEAT_ALL = 1;
/// 当前曲目循环播放。
export const AUDIO_REPEAT_ONE = 2;
