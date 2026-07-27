/// Location of a finding in the scanned UTF-8 source.
///
/// `start` and `end` are zero-based byte offsets.
/// `line` and `column` are one-based.
/// `column` counts Unicode scalar values, not bytes.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct Location {
    start: usize,
    end: usize,
    line: usize,
    column: usize,
}

impl Location {
    pub(crate) const fn from_span(start: usize, end: usize) -> Self {
        Self {
            start,
            end,
            line: 0,
            column: 0,
        }
    }

    pub(crate) const fn set_position(&mut self, line: usize, column: usize) {
        self.line = line;
        self.column = column;
    }

    #[must_use]
    pub const fn start(&self) -> usize {
        self.start
    }

    #[must_use]
    pub const fn end(&self) -> usize {
        self.end
    }

    #[must_use]
    pub const fn line(&self) -> usize {
        self.line
    }

    #[must_use]
    pub const fn column(&self) -> usize {
        self.column
    }
}