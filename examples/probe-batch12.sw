import { println, flush } from "std/io";
import {
    mean_int,
    mean_float,
    median_int,
    median_float,
    variance_int,
    variance_float,
    stdev_int,
    stdev_float,
    sum_int,
    sum_float,
    min_int,
    max_int,
    min_float,
    max_float,
} from "std/math";
import { format_float } from "std/string";
import {
    取平均值整数,
    取平均值小数,
    取中位数整数,
    取中位数小数,
    取方差整数,
    取方差小数,
    取标准差整数,
    取标准差小数,
    取数组和整数,
    取数组和小数,
    取数组最小值整数,
    取数组最大值整数,
    取数组最小值小数,
    取数组最大值小数,
} from "std/math";

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

    // ---------- 均值 ----------
    passed = passed & check(mean_int([1, 2, 3, 4]) == 2.5, "mean_int basic");
    passed = passed & check(mean_int([1, 2, 3]) == 2.0, "mean_int whole");
    passed = passed & check(mean_int([]) == 0.0, "mean_int empty");
    passed = passed & check(mean_int([0, 0, 0]) == 0.0, "mean_int zeros");
    passed = passed & check(mean_float([1.0, 2.0, 3.0]) == 2.0, "mean_float basic");
    passed = passed & check(mean_float([]) == 0.0, "mean_float empty");
    passed = passed & check(mean_float([2.5, 3.5]) == 3.0, "mean_float pair");

    // ---------- 中位数 ----------
    passed = passed & check(median_int([3, 1, 2]) == 2.0, "median_int odd");
    passed = passed & check(median_int([1, 2, 3, 4]) == 2.5, "median_int even");
    passed = passed & check(median_int([]) == 0.0, "median_int empty");
    passed = passed & check(median_int([7]) == 7.0, "median_int single");
    passed = passed & check(median_int([5, 1, 9, 3]) == 4.0, "median_int even mid");
    passed = passed & check(median_float([2.0, 1.0, 3.0]) == 2.0, "median_float odd");
    passed = passed & check(median_float([1.5, 2.5]) == 2.0, "median_float even");
    passed = passed & check(median_float([]) == 0.0, "median_float empty");
    passed = passed & check(median_float([4.0]) == 4.0, "median_float single");

    // ---------- 总体方差 / 标准差 ----------
    passed = passed & check(variance_int([2, 4, 4, 4, 5, 5, 7, 9]) == 4.0, "variance_int known");
    passed = passed & check(variance_int([1, 1, 1]) == 0.0, "variance_int constant");
    passed = passed & check(variance_int([]) == 0.0, "variance_int empty");
    passed = passed & check(variance_int([1, 2]) == 0.25, "variance_int pair");
    passed = passed & check(variance_float([2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0]) == 4.0, "variance_float known");
    passed = passed & check(variance_float([]) == 0.0, "variance_float empty");
    passed = passed & check(stdev_int([2, 4, 4, 4, 5, 5, 7, 9]) == 2.0, "stdev_int known");
    passed = passed & check(stdev_int([]) == 0.0, "stdev_int empty");
    passed = passed & check(stdev_int([5, 5, 5]) == 0.0, "stdev_int constant");
    passed = passed & check(stdev_float([2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0]) == 2.0, "stdev_float known");
    passed = passed & check(stdev_float([]) == 0.0, "stdev_float empty");

    // ---------- min / max / sum（转发 std/array） ----------
    passed = passed & check(min_int([3, 1, 2]) == 1, "min_int");
    passed = passed & check(max_int([3, 1, 2]) == 3, "max_int");
    passed = passed & check(sum_int([1, 2, 3]) == 6, "sum_int");
    passed = passed & check(min_int([]) == 0 && max_int([]) == 0 && sum_int([]) == 0, "int empty");
    passed = passed & check(min_float([3.5, 1.5, 2.5]) == 1.5, "min_float");
    passed = passed & check(max_float([3.5, 1.5, 2.5]) == 3.5, "max_float");
    passed = passed & check(sum_float([1.5, 2.5]) == 4.0, "sum_float");
    passed = passed & check(min_float([]) == 0.0 && max_float([]) == 0.0 && sum_float([]) == 0.0, "float empty");

    // ---------- 中文名转发 ----------
    passed = passed & check(取平均值整数([1, 2, 3, 4]) == 2.5, "取平均值整数");
    passed = passed & check(取中位数小数([1.5, 2.5]) == 2.0, "取中位数小数");
    passed = passed & check(取方差整数([2, 4, 4, 4, 5, 5, 7, 9]) == 4.0, "取方差整数");
    passed = passed & check(取标准差整数([2, 4, 4, 4, 5, 5, 7, 9]) == 2.0, "取标准差整数");
    passed = passed & check(取数组和整数([1, 2, 3]) == 6, "取数组和整数");
    passed = passed & check(取数组和小数([1.5, 2.5]) == 4.0, "取数组和小数");
    passed = passed & check(取数组最小值整数([3, 1, 2]) == 1, "取数组最小值整数");
    passed = passed & check(取数组最大值小数([3.5, 1.5, 2.5]) == 3.5, "取数组最大值小数");

    // ---------- 与平均值联动（文档示例展示） ----------
    const data = [10, 20, 30, 40, 50];
    passed = passed & check(mean_int(data) == 30.0, "doc mean");
    passed = passed & check(median_int(data) == 30.0, "doc median");
    passed = passed & check(sum_int(data) == 150, "doc sum");
    println(`stats: mean=${format_float(mean_int(data), 1)} median=${format_float(median_int(data), 1)} stdev=${format_float(stdev_int(data), 3)}`);
    flush();

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
