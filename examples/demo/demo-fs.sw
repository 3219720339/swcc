import { println } from "std/io";
import {
    write_all,
    append,
    read_all,
    read_lines,
    exists,
    path_join,
    path_basename,
    path_dirname,
    path_ext,
    list_dir,
    is_dir,
    mkdir_p,
    file_size_path,
    file_mtime,
    is_file,
    path_absolute,
    path_normalize,
    is_absolute,
    path_parts,
    expand_home,
    glob,
    walk_files,
    copy_file,
    touch,
} from "std/fs";
import { join } from "std/string";
import { date_string } from "std/time";

function main(): int {
    const dir = path_join(".", "demo-fs-tmp");
    mkdir_p(dir);
    const file = path_join(dir, "data.txt");
    write_all(file, "line one\nline two\n");
    append(file, "line three\n");
    println(`exists=${exists(file)} is_file=${is_file(file)} is_dir=${is_dir(dir)}`);
    println(`read_all=[${read_all(file)}]`);
    println(`lines=${read_lines(file).length} content=${join(read_lines(file), "|")}`);
    println(`file_size=${file_size_path(file)} mtime=${date_string(file_mtime(file))}`);
    const entries = list_dir(dir);
    println(`list_dir=${join(entries, ",")}`);
    println(`path_join=${path_join("a", "b")} basename=${path_basename("a/b/c.txt")} dirname=${path_dirname("a/b/c.txt")} ext=${path_ext("a/b/c.txt")}`);
    println(`path_absolute=${path_absolute(file)} normalize=${path_normalize("a/./b/../c")} is_absolute=${is_absolute("C:\\x") || is_absolute("/x")}`);
    println(`path_parts=${path_parts("a/b/c").length} expand_home=${expand_home("~")}`);
    copy_file(file, path_join(dir, "copy.txt"));
    println(`copy_exists=${exists(path_join(dir, "copy.txt"))}`);
    const walked = walk_files(dir);
    println(`walk_files_count=${walked.length}`);
    const matches = glob(path_join(dir, "*.txt"));
    println(`glob_count=${matches.length}`);
    touch(path_join(dir, "touched.txt"));
    println(`touch_exists=${exists(path_join(dir, "touched.txt"))}`);
    return 0;
}
