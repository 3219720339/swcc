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
#define SW_AUDIO_FAILED 4
#define SW_AUDIO_EVENT_ENDED 1
#define SW_AUDIO_EVENT_ERROR 2
#define SW_AUDIO_EVENT_DEVICE_LOST 3

#if defined(_WIN32) || (defined(__APPLE__) && __has_include(<AudioToolbox/AudioToolbox.h>)) || (defined(__linux__) && __has_include(<alsa/asoundlib.h>))
#include <string.h>
#if defined(_WIN32)
#include <windows.h>
#else
#include <pthread.h>
#include <unistd.h>
#endif
#if !defined(_WIN32)
#define MA_NO_WIN32
#endif
#define NDEBUG
#define MINIAUDIO_IMPLEMENTATION
#define MA_NO_JACK
#define MA_NO_PULSEAUDIO
// Sw runtime is linked without the CRT debug assertion entry point.
#define MA_ASSERT(expression) ((void)0)
#define assert(expression) ((void)0)
#include "vendor/miniaudio/miniaudio.h"

typedef struct { int code; ma_uint64 position_frames; } sw_audio_event;

typedef struct {
    ma_device device;
    ma_decoder decoder;
    ma_uint64 length_frames;
    volatile int used;
    volatile int state;
    volatile int volume;
    volatile int speed;
    volatile int seek_requested;
    volatile ma_uint64 seek_frame;
    volatile ma_uint64 position_frames;
    float held_left;
    float held_right;
    int phase_percent;
    volatile int error_code;
    char* queue_paths[4];
    int64_t queue_lengths[4];
    int queue_head;
    int queue_count;
    char* source_path;
    int64_t source_length;
    ma_device_id selected_device;
    int has_selected_device;
    volatile int control_running;
#if defined(_WIN32)
    HANDLE control_thread;
#else
    pthread_t control_thread;
#endif
    sw_audio_event events[16];
    volatile int event_head;
    volatile int event_count;
    volatile ma_uint64 last_event_position_frames;
} sw_audio_player;

#define SW_AUDIO_MAX_PLAYERS 8
static sw_audio_player sw_audio_players[SW_AUDIO_MAX_PLAYERS];
static volatile int sw_audio_default_device = -1;

static void sw_audio_event_push(sw_audio_player* player, int code) {
    int count = __atomic_load_n(&player->event_count, __ATOMIC_ACQUIRE);
    if (count >= 16) return;
    int head = __atomic_load_n(&player->event_head, __ATOMIC_ACQUIRE);
    int slot = (head + count) % 16;
    player->events[slot].code = code;
    player->events[slot].position_frames = __atomic_load_n(&player->position_frames, __ATOMIC_ACQUIRE);
    __atomic_fetch_add(&player->event_count, 1, __ATOMIC_RELEASE);
}

static int64_t audio_next(int64_t handle);

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
    if (__atomic_exchange_n(&player->seek_requested, 0, __ATOMIC_ACQ_REL)) {
        ma_uint64 target = __atomic_load_n(&player->seek_frame, __ATOMIC_ACQUIRE);
        ma_decoder_seek_to_pcm_frame(&player->decoder, target);
        __atomic_store_n(&player->position_frames, target, __ATOMIC_RELEASE);
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
                sw_audio_event_push(player, SW_AUDIO_EVENT_ENDED);
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

static void* sw_audio_control_loop(void* opaque) {
    sw_audio_player* player = (sw_audio_player*)opaque;
    while (__atomic_load_n(&player->control_running, __ATOMIC_ACQUIRE)) {
        if (__atomic_load_n(&player->state, __ATOMIC_ACQUIRE) == SW_AUDIO_ENDED && player->queue_count > 0) {
            audio_next((int64_t)(player - sw_audio_players + 1));
        }
#if defined(_WIN32)
        Sleep(20);
#else
        usleep(20000);
#endif
    }
    return 0;
}

static int sw_audio_control_start(sw_audio_player* player) {
    __atomic_store_n(&player->control_running, 1, __ATOMIC_RELEASE);
#if defined(_WIN32)
    player->control_thread = CreateThread(0, 0, (LPTHREAD_START_ROUTINE)sw_audio_control_loop, player, 0, 0);
    if (!player->control_thread) { __atomic_store_n(&player->control_running, 0, __ATOMIC_RELEASE); return -1; }
#else
    if (pthread_create(&player->control_thread, 0, sw_audio_control_loop, player) != 0) { __atomic_store_n(&player->control_running, 0, __ATOMIC_RELEASE); return -1; }
#endif
    return 0;
}

static void sw_audio_control_stop(sw_audio_player* player) {
    if (!__atomic_exchange_n(&player->control_running, 0, __ATOMIC_ACQ_REL)) return;
#if defined(_WIN32)
    WaitForSingleObject(player->control_thread, INFINITE); CloseHandle(player->control_thread); player->control_thread = 0;
#else
    pthread_join(player->control_thread, 0);
#endif
}

static sw_audio_player* sw_audio_get(int64_t handle) {
    if (handle <= 0 || handle > SW_AUDIO_MAX_PLAYERS) return 0;
    sw_audio_player* player = &sw_audio_players[handle - 1];
    return __atomic_load_n(&player->used, __ATOMIC_ACQUIRE) ? player : 0;
}
static int sw_audio_valid(int64_t handle) { return sw_audio_get(handle) != 0; }

static int sw_audio_decoder_open(sw_audio_player* player, const char* path, int64_t length) {
    ma_decoder_config config = ma_decoder_config_init(ma_format_f32, 2, 48000);
#if defined(_WIN32)
    if (length <= 0 || length > 32767) return -2;
    wchar_t wide[32768];
    int wide_len = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, path, (int)length, wide, 32767);
    if (wide_len <= 0) return -2;
    wide[wide_len] = 0;
    return ma_decoder_init_file_w(wide, &config, &player->decoder) == MA_SUCCESS ? 0 : -2;
#else
    (void)length;
    return ma_decoder_init_file(path, &config, &player->decoder) == MA_SUCCESS ? 0 : -2;
#endif
}

int64_t audio_open(sw_string* path) {
    if (path == 0 || path->data == 0 || path->len <= 0 || path->len > 32767) return -1;
    int slot = -1;
    for (int i = 0; i < SW_AUDIO_MAX_PLAYERS; i++) {
        if (!__atomic_load_n(&sw_audio_players[i].used, __ATOMIC_ACQUIRE)) { slot = i; break; }
    }
    if (slot < 0) return -1;
    sw_audio_player* player = &sw_audio_players[slot];
    player->error_code = sw_audio_decoder_open(player, path->data, path->len);
    if (player->error_code != 0) return -1;
    player->length_frames = ma_decoder_get_length_in_pcm_frames(&player->decoder);
    player->source_path = (char*)ma_malloc((size_t)path->len + 1, 0);
    if (!player->source_path) { ma_decoder_uninit(&player->decoder); player->error_code = -5; return -1; }
    memcpy(player->source_path, path->data, (size_t)path->len); player->source_path[path->len] = 0; player->source_length = path->len;
    player->volume = 100; player->speed = 100; player->state = SW_AUDIO_STOPPED;
    player->position_frames = 0; player->seek_requested = 0; player->seek_frame = 0; player->phase_percent = 100;
    player->held_left = 0.0f; player->held_right = 0.0f;
    player->queue_head = 0; player->queue_count = 0;
    player->event_head = 0; player->event_count = 0; player->has_selected_device = 0;
    if (sw_audio_default_device >= 0) {
        ma_context context; ma_device_info* playback = 0; ma_device_info* capture = 0; ma_uint32 playback_count = 0; ma_uint32 capture_count = 0;
        if (ma_context_init(0, 0, 0, &context) == MA_SUCCESS && ma_context_get_devices(&context, &playback, &playback_count, &capture, &capture_count) == MA_SUCCESS && (ma_uint32)sw_audio_default_device < playback_count) {
            memcpy(&player->selected_device, &playback[sw_audio_default_device].id, sizeof(ma_device_id)); player->has_selected_device = 1;
        }
        ma_context_uninit(&context);
    }
    ma_device_config device_config = ma_device_config_init(ma_device_type_playback);
    device_config.playback.format = ma_format_f32; device_config.playback.channels = 2;
    device_config.playback.pDeviceID = player->has_selected_device ? &player->selected_device : 0;
    device_config.sampleRate = 48000; device_config.dataCallback = sw_audio_callback; device_config.pUserData = player;
    if (ma_device_init(0, &device_config, &player->device) != MA_SUCCESS) { player->error_code = -3; ma_decoder_uninit(&player->decoder); ma_free(player->source_path, 0); player->source_path = 0; return -1; }
    __atomic_store_n(&player->used, 1, __ATOMIC_RELEASE);
    if (ma_device_start(&player->device) != MA_SUCCESS) { player->error_code = -4; __atomic_store_n(&player->used, 0, __ATOMIC_RELEASE); ma_device_uninit(&player->device); ma_decoder_uninit(&player->decoder); ma_free(player->source_path, 0); player->source_path = 0; return -1; }
    if (sw_audio_control_start(player) != 0) { player->error_code = -6; ma_device_uninit(&player->device); ma_decoder_uninit(&player->decoder); ma_free(player->source_path, 0); player->source_path = 0; return -1; }
    return slot + 1;
}
int64_t audio_play(int64_t handle) { sw_audio_player* player = sw_audio_get(handle); if (!player) return -1; if (__atomic_load_n(&player->state, __ATOMIC_ACQUIRE) == SW_AUDIO_ENDED) { __atomic_store_n(&player->seek_frame, 0, __ATOMIC_RELEASE); __atomic_store_n(&player->seek_requested, 1, __ATOMIC_RELEASE); } __atomic_store_n(&player->state, SW_AUDIO_PLAYING, __ATOMIC_RELEASE); return 0; }
int64_t audio_pause(int64_t handle) { sw_audio_player* player = sw_audio_get(handle); if (!player) return -1; __atomic_store_n(&player->state, SW_AUDIO_PAUSED, __ATOMIC_RELEASE); return 0; }
int64_t audio_resume(int64_t handle) { return audio_play(handle); }
int64_t audio_stop(int64_t handle) { sw_audio_player* player = sw_audio_get(handle); if (!player) return -1; __atomic_store_n(&player->state, SW_AUDIO_STOPPED, __ATOMIC_RELEASE); __atomic_store_n(&player->seek_frame, 0, __ATOMIC_RELEASE); __atomic_store_n(&player->seek_requested, 1, __ATOMIC_RELEASE); __atomic_store_n(&player->position_frames, 0, __ATOMIC_RELEASE); return 0; }
int64_t audio_close(int64_t handle) { sw_audio_player* player = sw_audio_get(handle); if (!player) return -1; __atomic_store_n(&player->state, SW_AUDIO_STOPPED, __ATOMIC_RELEASE); sw_audio_control_stop(player); ma_device_uninit(&player->device); ma_decoder_uninit(&player->decoder); for (int i = 0; i < 4; i++) { if (player->queue_paths[i]) { ma_free(player->queue_paths[i], 0); player->queue_paths[i] = 0; } } if (player->source_path) { ma_free(player->source_path, 0); player->source_path = 0; } __atomic_store_n(&player->used, 0, __ATOMIC_RELEASE); return 0; }
int64_t audio_last_error(int64_t handle) { sw_audio_player* player = sw_audio_get(handle); return player ? player->error_code : -1; }
int64_t audio_device_count(void) {
    ma_context context; ma_device_info* playback = 0; ma_device_info* capture = 0; ma_uint32 playback_count = 0; ma_uint32 capture_count = 0;
    if (ma_context_init(0, 0, 0, &context) != MA_SUCCESS) return -1;
    ma_result result = ma_context_get_devices(&context, &playback, &playback_count, &capture, &capture_count); ma_context_uninit(&context);
    return result == MA_SUCCESS ? (int64_t)playback_count : -1;
}
int64_t audio_state(int64_t handle) { sw_audio_player* player = sw_audio_get(handle); return player ? __atomic_load_n(&player->state, __ATOMIC_ACQUIRE) : -1; }
int64_t audio_position_ms(int64_t handle) { sw_audio_player* player = sw_audio_get(handle); return player ? (int64_t)(__atomic_load_n(&player->position_frames, __ATOMIC_ACQUIRE) * 1000 / 48000) : -1; }
int64_t audio_duration_ms(int64_t handle) { sw_audio_player* player = sw_audio_get(handle); return player ? (int64_t)(player->length_frames * 1000 / 48000) : -1; }
int64_t audio_progress_percent(int64_t handle) { sw_audio_player* player = sw_audio_get(handle); if (!player) return -1; ma_uint64 duration = player->length_frames; if (duration == 0) return 0; ma_uint64 position = __atomic_load_n(&player->position_frames, __ATOMIC_ACQUIRE); if (position >= duration) return 100; return (int64_t)(position * 100 / duration); }
extern sw_string* sw_string_from_literal(const char* data, int64_t len);
sw_string* audio_device_name(int64_t index) {
    if (index < 0) return sw_string_from_literal("", 0);
    ma_context context; ma_device_info* playback = 0; ma_device_info* capture = 0; ma_uint32 playback_count = 0; ma_uint32 capture_count = 0;
    if (ma_context_init(0, 0, 0, &context) != MA_SUCCESS) return sw_string_from_literal("", 0);
    sw_string* result = 0;
    if (ma_context_get_devices(&context, &playback, &playback_count, &capture, &capture_count) == MA_SUCCESS && index < (int64_t)playback_count) result = sw_string_from_literal(playback[index].name, (int64_t)strlen(playback[index].name));
    ma_context_uninit(&context); return result ? result : sw_string_from_literal("", 0);
}
int64_t audio_set_default_device(int64_t index) { if (index < -1 || index >= audio_device_count()) return -1; sw_audio_default_device = (int)index; return 0; }
int64_t audio_event_poll(int64_t handle) { sw_audio_player* player = sw_audio_get(handle); if (!player) return -1; int count = __atomic_load_n(&player->event_count, __ATOMIC_ACQUIRE); if (count <= 0) return 0; int head = __atomic_load_n(&player->event_head, __ATOMIC_ACQUIRE); int code = player->events[head].code; __atomic_store_n(&player->last_event_position_frames, player->events[head].position_frames, __ATOMIC_RELEASE); __atomic_store_n(&player->event_head, (head + 1) % 16, __ATOMIC_RELEASE); __atomic_fetch_sub(&player->event_count, 1, __ATOMIC_RELEASE); return code; }
int64_t audio_event_position_ms(int64_t handle) { sw_audio_player* player = sw_audio_get(handle); return player ? (int64_t)(__atomic_load_n(&player->last_event_position_frames, __ATOMIC_ACQUIRE) * 1000 / 48000) : -1; }
int64_t audio_seek(int64_t handle, int64_t position_ms) {
    sw_audio_player* player = sw_audio_get(handle); if (!player || position_ms < 0) return -1;
    ma_uint64 target = (ma_uint64)position_ms * 48000 / 1000;
    if (target > player->length_frames) target = player->length_frames;
    // The decoder is owned by the callback thread. Request the seek there to
    // avoid racing ma_decoder_read_pcm_frames from the Sw control thread.
    __atomic_store_n(&player->seek_frame, target, __ATOMIC_RELEASE);
    __atomic_store_n(&player->seek_requested, 1, __ATOMIC_RELEASE);
    __atomic_store_n(&player->position_frames, target, __ATOMIC_RELEASE);
    return 0;
}
int64_t audio_set_volume(int64_t handle, int64_t percent) { sw_audio_player* player = sw_audio_get(handle); if (!player || percent < 0 || percent > 200) return -1; __atomic_store_n(&player->volume, (int)percent, __ATOMIC_RELEASE); return 0; }
int64_t audio_set_speed(int64_t handle, int64_t speed) { sw_audio_player* player = sw_audio_get(handle); if (!player || speed < 25 || speed > 400) return -1; __atomic_store_n(&player->speed, (int)speed, __ATOMIC_RELEASE); return 0; }
int64_t audio_queue(int64_t handle, sw_string* path) { sw_audio_player* player = sw_audio_get(handle); if (!player || !path || !path->data || path->len <= 0 || path->len > 32767 || player->queue_count >= 4) return -1; char* copy = (char*)ma_malloc((size_t)path->len + 1, 0); if (!copy) return -1; memcpy(copy, path->data, (size_t)path->len); copy[path->len] = 0; int slot = (player->queue_head + player->queue_count) % 4; player->queue_paths[slot] = copy; player->queue_lengths[slot] = path->len; player->queue_count++; return 0; }
static int64_t audio_next(int64_t handle) { sw_audio_player* player = sw_audio_get(handle); if (!player || player->queue_count <= 0) return -1; ma_device_stop(&player->device); ma_decoder_uninit(&player->decoder); int slot = player->queue_head; char* path = player->queue_paths[slot]; int64_t length = player->queue_lengths[slot]; player->queue_paths[slot] = 0; player->queue_head = (slot + 1) % 4; player->queue_count--; player->error_code = sw_audio_decoder_open(player, path, length); ma_free(path, 0); if (player->error_code != 0) { __atomic_store_n(&player->state, SW_AUDIO_FAILED, __ATOMIC_RELEASE); sw_audio_event_push(player, SW_AUDIO_EVENT_ERROR); return -1; } player->length_frames = ma_decoder_get_length_in_pcm_frames(&player->decoder); player->position_frames = 0; player->seek_requested = 0; player->seek_frame = 0; player->phase_percent = 100; if (ma_device_start(&player->device) != MA_SUCCESS) { player->error_code = -4; __atomic_store_n(&player->state, SW_AUDIO_FAILED, __ATOMIC_RELEASE); sw_audio_event_push(player, SW_AUDIO_EVENT_DEVICE_LOST); return -1; } __atomic_store_n(&player->state, SW_AUDIO_PLAYING, __ATOMIC_RELEASE); return 0; }

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
int64_t audio_progress_percent(int64_t handle) { (void)handle; return -1; }
int64_t audio_seek(int64_t handle, int64_t position_ms) { (void)handle; (void)position_ms; return -1; }
int64_t audio_set_volume(int64_t handle, int64_t percent) { (void)handle; (void)percent; return -1; }
int64_t audio_set_speed(int64_t handle, int64_t speed) { (void)handle; (void)speed; return -1; }
int64_t audio_last_error(int64_t handle) { (void)handle; return -1; }
int64_t audio_device_count(void) { return -1; }
sw_string* audio_device_name(int64_t index) { (void)index; return 0; }
int64_t audio_set_default_device(int64_t index) { (void)index; return -1; }
int64_t audio_queue(int64_t handle, sw_string* path) { (void)handle; (void)path; return -1; }
int64_t audio_event_poll(int64_t handle) { (void)handle; return -1; }
int64_t audio_event_position_ms(int64_t handle) { (void)handle; return -1; }
#endif
