import { println } from "std/io";
import {
    list_dir,
    is_dir,
    mkdir,
    remove,
    rename,
    copy_file,
    write_all,
    exists,
    path_basename,
    path_dirname,
    path_ext,
} from "std/fs";

function main(): int {
    mkdir("dir-demo");
    write_all("dir-demo/a.txt", "hello");
    write_all("dir-demo/b.txt", "world");
    const entries = list_dir("dir-demo");
    let names = "";
    let i = 0;
    while (i < entries.length) {
        names = `${names}${entries[i]} `;
        i = i + 1;
    }
    println(`entries=${entries.length} [${names}]`);
    println(`is-dir=${is_dir("dir-demo")} is-file=${is_dir("dir-demo/a.txt")}`);
    rename("dir-demo/a.txt", "dir-demo/renamed.txt");
    copy_file("dir-demo/b.txt", "dir-demo/copy.txt");
    println(`base=${path_basename("dir-demo/renamed.txt")} dir=${path_dirname("dir-demo/renamed.txt")} ext=${path_ext("dir-demo/renamed.txt")}`);
    println(`copy-exists=${exists("dir-demo/copy.txt")}`);
    remove("dir-demo/copy.txt");
    println(`removed=${exists("dir-demo/copy.txt") == 0}`);
    return 0;
}
