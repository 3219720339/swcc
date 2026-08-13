use std::path::PathBuf;

/// 一份源文件：路径、UTF-8 文本和预计算的行起始偏移。
#[derive(Clone, Debug)]
pub struct Source {
    pub path: PathBuf,
    pub text: String,
    line_starts: Vec<usize>,
}

impl Source {
    pub fn new(path: PathBuf, text: String) -> Self {
        let mut line_starts = vec![0];
        for (index, byte) in text.bytes().enumerate() {
            if byte == b'\n' {
                line_starts.push(index + 1);
            }
        }
        Self {
            path,
            text,
            line_starts,
        }
    }

    /// 把字节偏移转换为 1 起始的行列。
    pub fn line_col(&self, offset: usize) -> (usize, usize) {
        let offset = offset.min(self.text.len());
        let line = match self.line_starts.binary_search(&offset) {
            Ok(index) => index,
            Err(index) => index - 1,
        };
        let line_start = self.line_starts[line];
        let column = self.text[line_start..offset].chars().count() + 1;
        (line + 1, column)
    }
}
