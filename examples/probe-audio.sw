import { println } from "std/io";
import { sleep_ms } from "std/time";
import { platform } from "std/os";
import { read_file_bytes } from "std/fs";
import { wav_info_bytes, wav_gain_bytes, wav_speed_bytes, wav_fade_in_bytes, wav_fade_out_bytes, wav_mix_bytes, wav_waveform_peaks,
    audio_open, audio_play, audio_pause, audio_resume, audio_stop, audio_close, audio_seek,
    audio_state, audio_position_ms, audio_duration_ms, audio_progress_percent, audio_set_volume, audio_set_speed,
    audio_metadata, audio_last_error, audio_device_count, audio_queue,
    AUDIO_STOPPED, AUDIO_PLAYING, AUDIO_PAUSED } from "std/audio";

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
        println(` [skip] audio device or demo MP3 is unavailable, error=${audio_last_error(handle)}`);
        return 1;
    }
    const metadata = audio_metadata(path);
    const raw_audio = read_file_bytes(path);
    println(`metadata: bytes=${raw_audio.length}, format=${metadata.format}, channels=${metadata.channels}, rate=${metadata.sample_rate}, bitrate=${metadata.bitrate_kbps}kbps, title=${metadata.title}`);
    println(`diagnostics: devices=${audio_device_count()}, last_error=${audio_last_error(handle)}`);
    println(`queue: add-next=${audio_queue(handle, path)}`);
    const second = audio_open(path);
    println(`multi-handle: second=${second}, independent=${second > 0 && audio_state(second) == AUDIO_STOPPED}`);
    if (second > 0) { audio_close(second); }
    println(`playback: duration=${audio_duration_ms(handle)}ms, start volume=0%, speed=100%, progress=${audio_progress_percent(handle)}%`);
    audio_set_volume(handle, 0); audio_set_speed(handle, 100);
    const started = audio_play(handle);
    let progress_tick = 0;
    while (progress_tick < 3) {
        sleep_ms(1000);
        println(`progress poll: state=${audio_state(handle)}, position=${audio_position_ms(handle)}ms, duration=${audio_duration_ms(handle)}ms, percent=${audio_progress_percent(handle)}%`);
        progress_tick++;
    }
    // Audible fade-in: the runtime volume is changed while the device plays.
    sleep_ms(1000); audio_set_volume(handle, 25); println("playback effect: fade-in 25%");
    sleep_ms(1000); audio_set_volume(handle, 50); println("playback effect: fade-in 50%");
    sleep_ms(1000); audio_set_volume(handle, 75); println("playback effect: fade-in 75%");
    sleep_ms(1000); audio_set_volume(handle, 100); println("playback effect: fade-in 100%");
    // Audible real-time speed changes.
    audio_set_speed(handle, 50); println("playback effect: speed 0.5x"); sleep_ms(3000);
    audio_set_speed(handle, 200); println("playback effect: speed 2x"); sleep_ms(3000);
    audio_set_speed(handle, 100); println("playback effect: speed 1x");
    const before_pause = audio_position_ms(handle);
    const paused = audio_pause(handle);
    println(`playback: started=${started}, position=${before_pause}ms, pause=${paused}, state=${audio_state(handle)}`);
    sleep_ms(1000);
    const frozen = audio_position_ms(handle);
    const resumed = audio_resume(handle);
    const seeked = audio_seek(handle, 30000);
    println(`playback: frozen=${frozen}ms, resume=${resumed}, seek=${seeked}, state=${audio_state(handle)}`);
    // Audible fade-out before closing.
    sleep_ms(3000); audio_set_volume(handle, 75); println("playback effect: fade-out 75%");
    sleep_ms(700); audio_set_volume(handle, 50); println("playback effect: fade-out 50%");
    sleep_ms(700); audio_set_volume(handle, 25); println("playback effect: fade-out 25%");
    sleep_ms(700); audio_set_volume(handle, 0); println("playback effect: fade-out 0%");
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
    println(`volume: gain=50%, sample[0]=${gain[44]}, sample[2]=${gain[46]}`);
    passed = passed & check(gain.length == wav.length && gain[44] == 64 && gain[46] == 191, "wav gain");
    const speed = wav_speed_bytes(wav, 200);
    const speed_info = wav_info_bytes(speed);
    const half_speed = wav_speed_bytes(wav, 50);
    println(`linear speed: 0.5x=${half_speed.length} bytes, 2x=${speed.length} bytes`);
    passed = passed & check(speed_info.valid && speed_info.data_size == 2 && speed.length == 46, "wav offline speed");
    const fade_in = wav_fade_in_bytes(wav, 1);
    println(`fade in: duration=1ms, first sample=${fade_in[44]}`);
    passed = passed & check(fade_in.length == wav.length && fade_in[44] == 128, "wav fade in");
    const fade_out = wav_fade_out_bytes(wav, 1);
    println(`fade out: duration=1ms, last sample=${fade_out[47]}`);
    passed = passed & check(fade_out.length == wav.length && fade_out[47] == 128, "wav fade out");
    const mixed = wav_mix_bytes(wav, wav, 50);
    println(`pcm mix: overlay=50%, bytes=${mixed.length}`);
    passed = passed & check(mixed.length == wav.length && mixed[44] == 0, "wav mix");
    const peaks = wav_waveform_peaks(wav, 2);
    println(`waveform peaks: [${peaks[0]}, ${peaks[1]}]`);
    passed = passed & check(peaks.length == 2 && peaks[0] == 100 && peaks[1] == 99, "wav waveform");
    const stereo16: u8[] = [];
    const stereo_header: int[] = [82, 73, 70, 70, 52, 0, 0, 0, 87, 65, 86, 69, 102, 109, 116, 32,
        16, 0, 0, 0, 1, 0, 2, 0, 68, 172, 0, 0, 0, 0, 0, 0, 4, 0, 16, 0,
        100, 97, 116, 97, 8, 0, 0, 0, 0, 128, 0, 128, 255, 127, 255, 127];
    for (const value of stereo_header) { stereo16.push(value as u8); }
    const stereo_info = wav_info_bytes(stereo16);
    passed = passed & check(stereo_info.valid && stereo_info.channels == 2 && stereo_info.sample_rate == 44100 && stereo_info.bits_per_sample == 16, "wav 16-bit stereo");
    passed = passed & play_demo();
    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
