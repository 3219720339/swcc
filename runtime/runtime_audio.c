// Unified native audio backend. This file deliberately never calls into Sw.
// miniaudio is MIT licensed and vendored under runtime/vendor/miniaudio.

typedef long long int64_t;
typedef unsigned long long uint64_t;
typedef unsigned int uint32_t;
typedef struct { char* data; int64_t len; } sw_string;

#define SW_AUDIO_STOPPED 0
#define SW_AUDIO_PLAYING 1
#define SW_AUDIO_PAUSED 2
#define SW_AUDIO_ENDED 3

#if defined(_WIN32)
#define NDEBUG
#define MINIAUDIO_IMPLEMENTATION
#define MA_NO_JACK
#define MA_NO_PULSEAUDIO
// Sw runtime is linked without the CRT debug assertion entry point.
#define MA_ASSERT(expression) ((void)0)
#define assert(expression) ((void)0)
#include "vendor/miniaudio/miniaudio.h"

typedef struct {
    ma_device device;
    ma_decoder decoder;
    ma_uint64 length_frames;
    volatile int used;
    volatile int state;
    volatile int volume;
    volatile int speed;
    volatile int seek_to_start;
    volatile ma_uint64 position_frames;
    float held_left;
    float held_right;
    int phase_percent;
} sw_audio_player;

static sw_audio_player sw_audio;

static void sw_audio_silence(float* out, ma_uint32 frames) {
    ma_uint64 samples = (ma_uint64)frames * 2;
    for (ma_uint64 i = 0; i < samples; i++) out[i] = 0.0f;
}

static void sw_audio_callback(ma_device* device, void* output, const void* input, ma_uint32 frames) {
    (void)device; (void)input;
    sw_audio_player* player = (sw_audio_player*)device->pUserData;
    float* out = (float*)output;
    sw_audio_silence(out, frames);
    if (__atomic_load_n(&player->state, __ATOMIC_ACQUIRE) != SW_AUDIO_PLAYING) return;
    if (__atomic_exchange_n(&player->seek_to_start, 0, __ATOMIC_ACQ_REL)) {
        ma_decoder_seek_to_pcm_frame(&player->decoder, 0);
        __atomic_store_n(&player->position_frames, 0, __ATOMIC_RELEASE);
        player->phase_percent = 100;
    }
    int speed = __atomic_load_n(&player->speed, __ATOMIC_ACQUIRE);
    int volume = __atomic_load_n(&player->volume, __ATOMIC_ACQUIRE);
    for (ma_uint32 frame = 0; frame < frames; frame++) {
        player->phase_percent += speed;
        while (player->phase_percent >= 100) {
            float sample[2];
            if (ma_decoder_read_pcm_frames(&player->decoder, sample, 1) != 1) {
                __atomic_store_n(&player->state, SW_AUDIO_ENDED, __ATOMIC_RELEASE);
                return;
            }
            player->held_left = sample[0]; player->held_right = sample[1];
            player->phase_percent -= 100;
            __atomic_fetch_add(&player->position_frames, 1, __ATOMIC_ACQ_REL);
        }
        out[frame * 2] = player->held_left * (float)volume / 100.0f;
        out[frame * 2 + 1] = player->held_right * (float)volume / 100.0f;
    }
}

static int sw_audio_valid(int64_t handle) { return handle == 1 && __atomic_load_n(&sw_audio.used, __ATOMIC_ACQUIRE); }

int64_t audio_open(sw_string* path) {
    if (path == 0 || path->data == 0 || path->len <= 0 || path->len > 32767 || sw_audio.used) return -1;
    wchar_t wide[32768];
    int wide_len = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, path->data, (int)path->len, wide, 32767);
    if (wide_len <= 0) return -1;
    wide[wide_len] = 0;
    ma_decoder_config decoder_config = ma_decoder_config_init(ma_format_f32, 2, 48000);
    if (ma_decoder_init_file_w(wide, &decoder_config, &sw_audio.decoder) != MA_SUCCESS) return -1;
    sw_audio.length_frames = ma_decoder_get_length_in_pcm_frames(&sw_audio.decoder);
    sw_audio.volume = 100; sw_audio.speed = 100; sw_audio.state = SW_AUDIO_STOPPED;
    sw_audio.position_frames = 0; sw_audio.seek_to_start = 0; sw_audio.phase_percent = 100;
    sw_audio.held_left = 0.0f; sw_audio.held_right = 0.0f;
    ma_device_config device_config = ma_device_config_init(ma_device_type_playback);
    device_config.playback.format = ma_format_f32; device_config.playback.channels = 2;
    device_config.sampleRate = 48000; device_config.dataCallback = sw_audio_callback; device_config.pUserData = &sw_audio;
    if (ma_device_init(0, &device_config, &sw_audio.device) != MA_SUCCESS) { ma_decoder_uninit(&sw_audio.decoder); return -1; }
    __atomic_store_n(&sw_audio.used, 1, __ATOMIC_RELEASE);
    if (ma_device_start(&sw_audio.device) != MA_SUCCESS) { __atomic_store_n(&sw_audio.used, 0, __ATOMIC_RELEASE); ma_device_uninit(&sw_audio.device); ma_decoder_uninit(&sw_audio.decoder); return -1; }
    return 1;
}
int64_t audio_play(int64_t handle) { if (!sw_audio_valid(handle)) return -1; if (__atomic_load_n(&sw_audio.state, __ATOMIC_ACQUIRE) == SW_AUDIO_ENDED) __atomic_store_n(&sw_audio.seek_to_start, 1, __ATOMIC_RELEASE); __atomic_store_n(&sw_audio.state, SW_AUDIO_PLAYING, __ATOMIC_RELEASE); return 0; }
int64_t audio_pause(int64_t handle) { if (!sw_audio_valid(handle)) return -1; __atomic_store_n(&sw_audio.state, SW_AUDIO_PAUSED, __ATOMIC_RELEASE); return 0; }
int64_t audio_resume(int64_t handle) { return audio_play(handle); }
int64_t audio_stop(int64_t handle) { if (!sw_audio_valid(handle)) return -1; __atomic_store_n(&sw_audio.state, SW_AUDIO_STOPPED, __ATOMIC_RELEASE); __atomic_store_n(&sw_audio.seek_to_start, 1, __ATOMIC_RELEASE); __atomic_store_n(&sw_audio.position_frames, 0, __ATOMIC_RELEASE); return 0; }
int64_t audio_close(int64_t handle) { if (!sw_audio_valid(handle)) return -1; __atomic_store_n(&sw_audio.used, 0, __ATOMIC_RELEASE); __atomic_store_n(&sw_audio.state, SW_AUDIO_STOPPED, __ATOMIC_RELEASE); ma_device_uninit(&sw_audio.device); ma_decoder_uninit(&sw_audio.decoder); return 0; }
int64_t audio_state(int64_t handle) { return sw_audio_valid(handle) ? __atomic_load_n(&sw_audio.state, __ATOMIC_ACQUIRE) : -1; }
int64_t audio_position_ms(int64_t handle) { return sw_audio_valid(handle) ? (int64_t)(__atomic_load_n(&sw_audio.position_frames, __ATOMIC_ACQUIRE) * 1000 / 48000) : -1; }
int64_t audio_duration_ms(int64_t handle) { return sw_audio_valid(handle) ? (int64_t)(sw_audio.length_frames * 1000 / 48000) : -1; }
int64_t audio_set_volume(int64_t handle, int64_t percent) { if (!sw_audio_valid(handle) || percent < 0 || percent > 200) return -1; __atomic_store_n(&sw_audio.volume, (int)percent, __ATOMIC_RELEASE); return 0; }
int64_t audio_set_speed(int64_t handle, int64_t speed) { if (!sw_audio_valid(handle) || speed < 25 || speed > 400) return -1; __atomic_store_n(&sw_audio.speed, (int)speed, __ATOMIC_RELEASE); return 0; }

#else
// Kept linkable for static Linux and macOS cross targets until their device
// backends are packaged without imposing host audio-library dependencies.
int64_t audio_open(sw_string* path) { (void)path; return -1; }
int64_t audio_play(int64_t handle) { (void)handle; return -1; }
int64_t audio_pause(int64_t handle) { (void)handle; return -1; }
int64_t audio_resume(int64_t handle) { (void)handle; return -1; }
int64_t audio_stop(int64_t handle) { (void)handle; return -1; }
int64_t audio_close(int64_t handle) { (void)handle; return -1; }
int64_t audio_state(int64_t handle) { (void)handle; return -1; }
int64_t audio_position_ms(int64_t handle) { (void)handle; return -1; }
int64_t audio_duration_ms(int64_t handle) { (void)handle; return -1; }
int64_t audio_set_volume(int64_t handle, int64_t percent) { (void)handle; (void)percent; return -1; }
int64_t audio_set_speed(int64_t handle, int64_t speed) { (void)handle; (void)speed; return -1; }
#endif
