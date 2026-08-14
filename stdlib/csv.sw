// ===========================================================================
// std/csv —— CSV 行解析与连接
//
// 用法：
//   import { csv_parse_line, csv_join } from "std/csv";
//   const row = csv_parse_line("a,b,\"c,d\"");   // ["a","b","c,d"]
//   csv_join(["a", "b", "c,d"])                  // "a,b,\"c,d\""
//
// csv_parse_line 支持双引号包裹（引号内逗号/换行原样保留，"" 转义双引号）；
// csv_join 在字段含逗号/引号/换行时自动加引号并转义。
// ===========================================================================

/// 解析一行 CSV 为 string[]（支持引号包裹与 "" 转义）。
export extern c function csv_parse_line(text: string): string[];

/// 把 string[] 连接为一行 CSV（需要时自动加引号转义）。
export extern c function csv_join(items: string[]): string;

// ---------------------------------------------------------------------------
// 中文函数名（转发到英文实现，火山风格命名）
// ---------------------------------------------------------------------------

export function 解析CSV行(text: string): string[] {
    return csv_parse_line(text);
}

export function 生成CSV行(items: string[]): string {
    return csv_join(items);
}

/// 解析整个 CSV 文本为 string[][]（每行一个 string[]，自动跳过空行）。
export function csv_read_all(text: string): string[][] {
    const result: string[][] = [];
    for (const line of text.lines()) {
        if (line.trim() == "") {
            continue;
        }
        result.push(csv_parse_line(line));
    }
    return result;
}

/// 把 string[][] 连接为完整 CSV 文本（每行末尾 \r\n）。
export function csv_write_all(rows: string[][]): string {
    let result = "";
    for (const row of rows) {
        result = result + csv_join(row) + "\r\n";
    }
    return result;
}

export function 解析CSV文本(text: string): string[][] {
    return csv_read_all(text);
}

export function 生成CSV文本(rows: string[][]): string {
    return csv_write_all(rows);
}
