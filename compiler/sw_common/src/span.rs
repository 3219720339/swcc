/// 源码中的字节区间，`start` 与 `end` 都是相对源文件 UTF-8 文本的字节偏移。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub fn empty(at: usize) -> Self {
        Self { start: at, end: at }
    }

    /// 合并两个区间，覆盖从较早起点到较晚终点。
    pub fn merge(self, other: Span) -> Span {
        Span {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }
}
