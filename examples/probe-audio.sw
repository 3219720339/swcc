import { println } from "std/io";
import { wav_info_bytes, wav_gain_bytes, wav_speed_bytes } from "std/audio";

function check(condition: bool, label: string): int {
    if (condition) { println(`[ok] ${label}`); return 1; }
    println(`[FAIL] ${label}`); return 0;
}

function main(): int {
    // PCM 8-bit mono, 8 kHz, 4 samples：44 字节头 + 4 字节 data。
    const wav: u8[] = [];
    const header: int[] = [82, 73, 70, 70, 40, 0, 0, 0, 87, 65, 86, 69, 102, 109, 116, 32,
        16, 0, 0, 0, 1, 0, 1, 0, 64, 31, 0, 0, 64, 31, 0, 0, 1, 0, 8, 0,
        100, 97, 116, 97, 4, 0, 0, 0, 0, 128, 255, 128];
    for (const value of header) { wav.push(value as u8); }
    const info = wav_info_bytes(wav);
    let passed = check(info.valid && info.channels == 1 && info.sample_rate == 8000 && info.bits_per_sample == 8 && info.data_size == 4, "wav metadata");
    const gain = wav_gain_bytes(wav, 50);
    passed = passed & check(gain.length == wav.length && gain[44] == 64 && gain[46] == 191, "wav gain");
    const speed = wav_speed_bytes(wav, 200);
    const speed_info = wav_info_bytes(speed);
    passed = passed & check(speed_info.valid && speed_info.data_size == 2 && speed.length == 46, "wav offline speed");
    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
