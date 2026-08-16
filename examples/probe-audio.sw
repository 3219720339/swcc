import { println } from "std/io";
import { sleep_ms } from "std/time";
import { platform } from "std/os";
import { wav_info_bytes, wav_gain_bytes, wav_speed_bytes, wav_fade_in_bytes, wav_fade_out_bytes, wav_mix_bytes, wav_waveform_peaks,
    audio_open, audio_play, audio_pause, audio_resume, audio_stop, audio_close, audio_seek,
    audio_state, audio_position_ms, audio_duration_ms, audio_set_volume, audio_set_speed,
    AUDIO_PLAYING, AUDIO_PAUSED } from "std/audio";

function check(condition: bool, label: string): int {
    if (condition) { println(`[ok] ${label}`); return 1; }
    println(`[FAIL] ${label}`); return 0;
}

function play_demo(): int {
    if (platform() != "windows") {
        println("[skip] playback demo requires the native Windows audio backend");
        return 1;
    }
    const path = "C:\\Users\\Administrator\\Music\\一颗狼星 - 白嫁衣_L.mp3";
    const handle = audio_open(path);
    if (handle <= 0) {
        println("[skip] audio device or demo MP3 is unavailable");
        return 1;
    }
    println(`playback: duration=${audio_duration_ms(handle)}ms, volume=${audio_set_volume(handle, 100)}, speed=${audio_set_speed(handle, 100)}`);
    const started = audio_play(handle);
    sleep_ms(3000);
    const before_pause = audio_position_ms(handle);
    const paused = audio_pause(handle);
    println(`playback: started=${started}, position=${before_pause}ms, pause=${paused}, state=${audio_state(handle)}`);
    sleep_ms(1000);
    const frozen = audio_position_ms(handle);
    const resumed = audio_resume(handle);
    const seeked = audio_seek(handle, 30000);
    println(`playback: frozen=${frozen}ms, resume=${resumed}, seek=${seeked}, state=${audio_state(handle)}`);
    // Keep the process alive long enough to hear the resumed and seeked audio.
    sleep_ms(10000);
    const final_state = audio_state(handle);
    audio_stop(handle); audio_close(handle);
    return check(paused == 0 && resumed == 0 && seeked == 0 && final_state >= AUDIO_PLAYING && frozen == before_pause, "playback controls");
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
    passed = passed & play_demo();
    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
