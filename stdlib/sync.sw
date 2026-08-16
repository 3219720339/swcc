// std/sync - 跨平台同步原语。
//
// 当前提供原生句柄，不共享 GC 管理对象：channel 在发送端复制 string/u8[]，
// 接收端创建新对象。timeout_ms: -1 无限等待，0 立即返回，>0 为毫秒。
/// 创建互斥锁；失败返回 -1。
export extern c function mutex_new(): int;
/// 获取锁。0 成功，1 超时，-1 无效/已关闭。
export extern c function mutex_lock(mutex: int, timeout_ms: int): int;
/// 尝试获取锁。1 成功，0 当前被占用，-1 无效/已关闭。
export extern c function mutex_try_lock(mutex: int): int;
/// 释放锁。0 成功，-1 无效/已关闭。
export extern c function mutex_unlock(mutex: int): int;
/// 关闭锁。关闭后不能再次使用；不会立即释放底层内存，保证并发调用安全。
export extern c function mutex_close(mutex: int): int;
/// 创建条件变量；失败返回 -1。
export extern c function cond_new(): int;
/// 原子地释放 mutex 并等待 signal/broadcast，返回前重新取得 mutex。
/// 返回 0 已唤醒，1 超时，-1 无效。
export extern c function cond_wait(cond: int, mutex: int, timeout_ms: int): int;
export extern c function cond_signal(cond: int): int;
export extern c function cond_broadcast(cond: int): int;
export extern c function cond_close(cond: int): int;
/// 创建 64 位原子计数器；失败返回 -1。
export extern c function atomic_new(initial: int): int;
export extern c function atomic_load(atomic: int): int;
export extern c function atomic_store(atomic: int, value: int): int;
/// 原子加 delta，返回更新后的值；失败返回 -1。
export extern c function atomic_add(atomic: int, delta: int): int;
/// 比较并交换。1 成功，0 当前值不等于 expected，-1 无效。
export extern c function atomic_compare_exchange(atomic: int, expected: int, replacement: int): int;
export extern c function atomic_close(atomic: int): int;
/// 创建字符串 channel。capacity=0 为无界队列；失败返回 -1。
export extern c function channel_string_new(capacity: int): int;
/// 发送文本。0 成功，1 队列满而超时，-1 已关闭/无效。
export extern c function channel_string_send(channel: int, value: string, timeout_ms: int): int;
/// 接收文本；超时或关闭且队列为空时返回 null。
export extern c function channel_string_recv(channel: int, timeout_ms: int): string?;
/// 创建字节 channel。发送时复制 u8[]，接收时产生新的 u8[]。
export extern c function channel_bytes_new(capacity: int): int;
export extern c function channel_bytes_send(channel: int, value: u8[], timeout_ms: int): int;
export extern c function channel_bytes_recv(channel: int, timeout_ms: int): u8[]?;
/// 当前排队消息数；无效 channel 返回 -1。
export extern c function channel_len(channel: int): int;
/// 关闭 channel；仍可接收已经入队的消息。
export extern c function channel_close(channel: int): int;
/// 运行时并发基础自检：创建原生 worker、执行停世界 GC 并校验原子访问。
/// 0 表示通过，-1 表示失败；用于部署/CI 诊断。
export extern c function sync_runtime_self_test(): int;
export function mutex_lock_forever(mutex: int): int {
    return mutex_lock(mutex, - 1);
}
export function channel_string_send_forever(channel: int, value: string): int {
    return channel_string_send(channel, value, - 1);
}
export function channel_bytes_send_forever(channel: int, value: u8[]): int {
    return channel_bytes_send(channel, value, - 1);
}
