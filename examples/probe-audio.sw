import { println } from "std/io";
import { wav_info_bytes, wav_gain_bytes, wav_speed_bytes, wav_fade_in_bytes, wav_fade_out_bytes, wav_mix_bytes, wav_waveform_peaks } from "std/audio";

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
    const half_speed = wav_speed_bytes(wav, 50);
    println(`linear speed: 0.5x=${half_speed.length} bytes, 2x=${speed.length} bytes`);
    passed = passed & check(speed_info.valid && speed_info.data_size == 2 && speed.length == 46, "wav offline speed");
    const fade_in = wav_fade_in_bytes(wav, 1);
    passed = passed & check(fade_in.length == wav.length && fade_in[44] == 128, "wav fade in");
    const fade_out = wav_fade_out_bytes(wav, 1);
    passed = passed & check(fade_out.length == wav.length && fade_out[47] == 128, "wav fade out");
    const mixed = wav_mix_bytes(wav, wav, 50);
    println(`pcm mix: overlay=50%, bytes=${mixed.length}`);
    passed = passed & check(mixed.length == wav.length && mixed[44] == 0, "wav mix");
    const peaks = wav_waveform_peaks(wav, 2);
    println(`waveform peaks: [${peaks[0]}, ${peaks[1]}]`);
    passed = passed & check(peaks.length == 2 && peaks[0] == 100 && peaks[1] == 99, "wav waveform");
    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
