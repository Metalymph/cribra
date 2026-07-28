//! Source location model used by public findings.

/// Exact location of a detected span in a UTF-8 source string.
///
/// `start` and `end` are zero-based byte offsets and follow Rust's standard
/// half-open range convention: `start..end`.
///
/// `line` and `column` are one-based human-readable coordinates. Columns count
/// Unicode scalar values, not UTF-8 bytes and not grapheme clusters.
///
/// A location produced by [`Scanner::scan`](crate::Scanner::scan) always refers
/// to valid UTF-8 character boundaries in the scanned source.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
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
            line: 1,
            column: 1,
        }
    }

    pub(crate) const fn set_position(&mut self, line: usize, column: usize) {
        self.line = line;
        self.column = column;
    }

    /// Returns the zero-based byte offset at which the finding begins.
    #[must_use]
    pub const fn start(&self) -> usize {
        self.start
    }

    /// Returns the zero-based exclusive byte offset at which the finding ends.
    ///
    /// The matched source span is therefore `start()..end()`.
    #[must_use]
    pub const fn end(&self) -> usize {
        self.end
    }

    /// Returns the one-based source line containing the start of the finding.
    #[must_use]
    pub const fn line(&self) -> usize {
        self.line
    }

    /// Returns the one-based Unicode-scalar column containing the start of the
    /// finding.
    ///
    /// This value counts Unicode scalar values (`char` in Rust), not bytes and
    /// not user-perceived grapheme clusters.
    #[must_use]
    pub const fn column(&self) -> usize {
        self.column
    }

    /// Returns the length of the matched span in bytes.
    #[must_use]
    pub const fn byte_len(&self) -> usize {
        self.end - self.start
    }

    /// Returns `true` when the represented byte span is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// Returns the represented half-open byte range.
    #[must_use]
    pub const fn byte_range(&self) -> std::ops::Range<usize> {
        self.start..self.end
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_half_open_byte_span_and_position() {
        let mut location = Location::from_span(5, 11);
        location.set_position(3, 7);

        assert_eq!(location.start(), 5);
        assert_eq!(location.end(), 11);
        assert_eq!(location.byte_len(), 6);
        assert_eq!(location.byte_range(), 5..11);
        assert_eq!(location.line(), 3);
        assert_eq!(location.column(), 7);
        assert!(!location.is_empty());
    }

    #[test]
    fn detects_empty_span() {
        assert!(Location::from_span(4, 4).is_empty());
    }
}
