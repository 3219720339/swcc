// probe-ns.sw — 命名空间全量导入验证：`import * as ns from "std/xxx"` 对
// 所有标准库生效（常量/函数/函数作值/类型/泛型）。
import * as s from "std/string";
import * as t from "std/time";
import * as io from "std/io";
import * as arr from "std/array";
import * as m from "std/math";
import * as audio from "std/audio";

function main(): int {
    // 函数调用
    const up = s.to_upper("ab");
    const joined = s.join(["x", "y"], "-");
    const ms = t.now_ms();
    const nums = [3, 1, 2];
    arr.sort_int(nums);
    // 常量（若有）与数学函数
    const mx = m.max(7, 9);
    // 函数作值（回调）
    const apply = (f: ((int) => int), v: int): int => f(v);
    const double = (x: int): int => x * 2;
    const r = apply(double, 21);
    // 结构体类型经命名空间
    const w: audio.WavInfo = {
        valid: false, format: "", channels: 0, sample_rate: 0, bits_per_sample: 0,
        data_offset: 0, data_size: 0, duration_ms: 0
    };
    // 校验
    io.println("up=" + up + " join=" + joined + " sorted=" + nums[0] + "," + nums[1] + "," + nums[2]);
    io.println("ms>0=" + (ms > 0) + " max=" + mx + " apply=" + r);
    if (up != "AB" || joined != "x-y" || nums[0] != 1 || nums[2] != 3 || mx != 9 || r != 42) {
        io.println("FAIL");
        return 1;
    }
    io.println("PASS");
    return 0;
}
