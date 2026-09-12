//! Borrowed source context used by contextual validators.

use std::ops::Range;

/// Borrowed UTF-8 source context for validating one candidate span.
#[derive(Debug, Clone)]
pub(crate) struct ValidationContext<'a> {
    source: &'a str,
    candidate: Range<usize>,
}

impl<'a> ValidationContext<'a> {
    /// Creates context for `candidate` inside `source`.
    pub(crate) fn new(source: &'a str, candidate: Range<usize>) -> Self {
        debug_assert!(candidate.start <= candidate.end);
        debug_assert!(candidate.end <= source.len());
        debug_assert!(source.is_char_boundary(candidate.start));
        debug_assert!(source.is_char_boundary(candidate.end));

        Self { source, candidate }
    }

    /// Returns the candidate text.
    pub(crate) fn candidate(&self) -> &'a str {
        &self.source[self.candidate.clone()]
    }

    /// Returns at most `maximum_bytes` immediately preceding bytes while
    /// preserving UTF-8 boundaries.
    pub(crate) fn before_window(&self, maximum_bytes: usize) -> &'a str {
        let mut start = self.candidate.start.saturating_sub(maximum_bytes);

        while start < self.candidate.start && !self.source.is_char_boundary(start) {
            start += 1;
        }

        &self.source[start..self.candidate.start]
    }

    /// Returns at most `maximum_bytes` immediately following the candidate while
    /// preserving UTF-8 boundaries.
    pub(crate) fn after_window(&self, maximum_bytes: usize) -> &'a str {
        let mut end = self
            .candidate
            .end
            .saturating_add(maximum_bytes)
            .min(self.source.len());

        while end > self.candidate.end && !self.source.is_char_boundary(end) {
            end -= 1;
        }

        &self.source[self.candidate.end..end]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_candidate_and_utf8_safe_surrounding_windows() {
        let source = "αβγ password=secret τέλος";
        let start = source
            .find("secret")
            .expect("fixture must contain candidate");
        let context = ValidationContext::new(source, start..start + "secret".len());

        assert_eq!(context.candidate(), "secret");
        assert!(context.before_window(7).is_char_boundary(0));
        assert!(context.after_window(7).is_char_boundary(0));
    }
}
