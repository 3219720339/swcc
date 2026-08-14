import { println } from "std/io";
import {
    sort_int,
    sort_float,
    sort_string,
    sort_int_desc,
    sort_float_desc,
    sort_string_desc,
    reverse_int,
    reverse_float,
    reverse_string,
    min_int,
    max_int,
    sum_int,
    min_float,
    max_float,
    sum_float,
    unique_string,
    contains_int,
    contains_float,
    contains_string,
    index_of_int,
    index_of_float,
    index_of_string,
} from "std/array";
import { join } from "std/string";
import { format_float } from "std/string";

function is(nums: int[]): string {
    let text = "";
    let i = 0;
    while (i < nums.length) {
        if (i > 0) {
            text = text + ",";
        }
        text = text + nums[i];
        i = i + 1;
    }
    return "[" + text + "]";
}

function ss(items: string[]): string {
    return "[" + join(items, ",") + "]";
}

function main(): int {
    const nums = [5, 1, 4, 2, 3];
    sort_int(nums);
    println(`sort_int=${is(nums)}`);
    sort_int_desc(nums);
    println(`sort_int_desc=${is(nums)}`);
    reverse_int(nums);
    println(`reverse_int=${is(nums)}`);
    println(`min_int=${min_int(nums)} max_int=${max_int(nums)} sum_int=${sum_int(nums)}`);
    println(`contains_int=${contains_int(nums, 5)} index_of_int=${index_of_int(nums, 2)}`);

    const floats = [3.5, 1.2, 2.8];
    sort_float(floats);
    println(`sort_float=[${format_float(floats[0], 1)},${format_float(floats[1], 1)},${format_float(floats[2], 1)}]`);
    println(`min_float=${format_float(min_float(floats), 1)} max_float=${format_float(max_float(floats), 1)} sum_float=${format_float(sum_float(floats), 1)}`);
    println(`contains_float=${contains_float(floats, 2.8)} index_of_float=${index_of_float(floats, 2.8)}`);

    const words = ["banana", "apple", "cherry"];
    sort_string(words);
    println(`sort_string=${ss(words)}`);
    sort_string_desc(words);
    println(`sort_string_desc=${ss(words)}`);
    reverse_string(words);
    println(`reverse_string=${ss(words)}`);
    println(`unique_string=${ss(unique_string(["a", "b", "a", "c"]))}`);
    println(`contains_string=${contains_string(words, "apple")} index_of_string=${index_of_string(words, "apple")}`);
    return 0;
}
