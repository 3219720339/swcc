import {
    println
}
from "std/io";
import {
    mutex_new,
    mutex_lock,
    mutex_try_lock,
    mutex_unlock,
    mutex_close,
    cond_new,
    cond_wait,
    cond_signal,
    cond_close,
    atomic_new,
    atomic_load,
    atomic_store,
    atomic_add,
    atomic_compare_exchange,
    atomic_close,
    channel_string_new,
    channel_string_send,
    channel_string_recv,
    channel_bytes_new,
    channel_bytes_send,
    channel_bytes_recv,
    channel_len,
    channel_close,
    sync_runtime_self_test
}
from "std/sync";
function check(condition: bool, label: string): int {
    if (condition) {
        println(`[ok] ${label}`);
        return 1;
    }
    println(`[FAIL] ${label}`);
    return 0;
}
function main(): int {
    let passed = 1;
    passed = passed & check(sync_runtime_self_test() == 0, "runtime thread registration and stop-the-world gc");
    const mutex = mutex_new();
    passed = passed & check(mutex > 0 && mutex_lock(mutex, - 1) == 0, "mutex lock");
    passed = passed & check(mutex_try_lock(mutex) == 0 && mutex_unlock(mutex) == 0, "mutex try and unlock");
    const cond = cond_new();
    passed = passed & check(mutex_lock(mutex, - 1) == 0 && cond_wait(cond, mutex, 0) == 1 && mutex_unlock(mutex) == 0, "condition timeout relocks mutex");
    passed = passed & check(cond_signal(cond) == 0 && cond_close(cond) == 0 && mutex_close(mutex) == 0, "condition signal and close");
    const atomic = atomic_new(3);
    passed = passed & check(atomic_load(atomic) == 3 && atomic_add(atomic, 4) == 7, "atomic add");
    passed = passed & check(atomic_compare_exchange(atomic, 7, 9) == 1 && atomic_compare_exchange(atomic, 7, 11) == 0 && atomic_store(atomic, 12) == 0 && atomic_load(atomic) == 12, "atomic compare exchange");
    passed = passed & check(atomic_close(atomic) == 0, "atomic close");
    const text_channel = channel_string_new(1);
    passed = passed & check(channel_string_send(text_channel, "hello", 0) == 0 && channel_len(text_channel) == 1, "string channel send");
    passed = passed & check(channel_string_send(text_channel, "full", 0) == 1 && (channel_string_recv(text_channel, 0) ?? "") == "hello", "string channel timeout and receive");
    passed = passed & check(channel_close(text_channel) == 0 && channel_string_recv(text_channel, 0) == null, "string channel close");
    const byte_channel = channel_bytes_new(1);
    const source: u8[] = [65 as u8, 66 as u8];
    passed = passed & check(channel_bytes_send(byte_channel, source, 0) == 0 && channel_len(byte_channel) == 1, "bytes channel send");
    const received = channel_bytes_recv(byte_channel, 0);
    passed = passed & check(received != null, "bytes channel receive");
    passed = passed & check(channel_close(byte_channel) == 0 && channel_bytes_recv(byte_channel, 0) == null, "bytes channel close");
    let text = "seed";
    let i = 0;
    while (i < 16000) {
        text = text + "x";
        i++;
    }
    passed = passed & check(text.length == 16004, "gc allocation safepoint");
    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1?0: 1;
}
