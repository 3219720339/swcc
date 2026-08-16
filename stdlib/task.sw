// std/task - Task<int> / Future 基础。
//
// Task 句柄是 int。`await task` 等同于 task_await(task)，等待期间持续参与 GC
// safepoint。取消为协作式：任务启动前必定生效，运行中的闭包应轮询 task_cancelled()。
export const TASK_RUNNING = 1;
export const TASK_COMPLETED = 2;
export const TASK_FAILED = 3;
export const TASK_CANCELLED = 4;
/// 创建 Task<int>。callback 必须是无参数、返回 int 的闭包；失败返回 -1。
export extern c function task_spawn(callback: any): int;
export extern c function task_poll(task: int): int;
/// 等待并回收任务线程。0 完成，1 超时，-1 无效或已 detach。
export extern c function task_join(task: int, timeout_ms: int): int;
/// 等待任务并取得正常结果；失败或取消返回 0，配合 task_state 检查。
export extern c function task_await(task: int): int;
export extern c function task_detach(task: int): int;
export extern c function task_state(task: int): int;
export extern c function task_result(task: int): int;
export extern c function task_exception_type(task: int): int;
/// 请求协作取消。1 已请求，0 已结束，-1 无效。
export extern c function task_cancel(task: int): int;
/// 仅在 task 闭包内调用；1 表示已经收到取消请求。
export extern c function task_cancelled(): int;
export function task_join_forever(task: int): int {
    return task_join(task, - 1);
}
