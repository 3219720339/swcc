// Result 类型（Rust 风格错误处理）。
// 配合 `?` 运算符：`value?` 在 Err 时提前返回 Err，在 Ok 时取出 payload。
// 用法：
//   import { Result, Ok, Err } from "std/result";
//   function parse(text: string): Result<int, string> { ... return Result.Ok(42); ... }
//   function main(): int {
//       const value = parse("42")?;   // Err 时直接返回，Ok 时 value = 42
//       ...
//   }
export enum Result<T, E> {
    Ok(T),
    Err(E),
}
