import {
    list_dir,
    is_dir,
    mkdir,
    remove,
    rename,
    copy_file,
    path_basename,
    path_dirname,
    path_ext,
} from "std/fs";

function main(): int {
    const entries = list_dir(".");
    const base = path_basename("a/b.txt");
    const dir = path_dirname("a/b.txt");
    const ext = path_ext("a/b.txt");
    if (base == "b.txt" && dir == "a" && ext == ".txt" && entries.length >= 0) {
        return 0;
    }
    return 1;
}
