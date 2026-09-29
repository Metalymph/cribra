//! Incremental source-global coordinate tracking.
//!
//! Coordinates advance over validated UTF-8 text independently of transport
//! partitioning. Byte offsets are zero-based; line and Unicode-scalar columns
//! are one-based, matching Cribra's public [`Location`](crate::Location)
//! contract.

/// Source-global position immediately following all text accepted so far.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) struct SourceCoordinates {
    byte_offset: usize,
    line: usize,
    column: usize,
}

impl Default for SourceCoordinates {
    fn default() -> Self {
        Self {
            byte_offset: 0,
            line: 1,
            column: 1,
        }
    }
}

impl SourceCoordinates {
    /// Advances coordinates over one complete UTF-8 segment.
    ///
    /// Newlines are defined by `\n`, matching the existing whole-source
    /// scanner contract. Other Unicode scalar values, including `\r`,
    /// advance the current column by one.
    pub(crate) fn advance(&mut self, text: &str) {
        self.byte_offset += text.len();

        for character in text.chars() {
            if character == '\n' {
                self.line += 1;
                self.column = 1;
            } else {
                self.column += 1;
            }
        }
    }

    pub(crate) const fn byte_offset(&self) -> usize {
        self.byte_offset
    }

    pub(crate) const fn line(&self) -> usize {
        self.line
    }

    pub(crate) const fn column(&self) -> usize {
        self.column
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utf8_transport::Utf8Transport;

    #[test]
    fn starts_at_public_source_origin() {
        let coordinates = SourceCoordinates::default();

        assert_eq!(coordinates.byte_offset(), 0);
        assert_eq!(coordinates.line(), 1);
        assert_eq!(coordinates.column(), 1);
    }

    #[test]
    fn ascii_advances_bytes_and_scalar_columns() {
        let mut coordinates = SourceCoordinates::default();

        coordinates.advance("hello");

        assert_eq!(coordinates.byte_offset(), 5);
        assert_eq!(coordinates.line(), 1);
        assert_eq!(coordinates.column(), 6);
    }

    #[test]
    fn unicode_columns_count_scalars_not_bytes() {
        let mut coordinates = SourceCoordinates::default();

        coordinates.advance("aé€🦀");

        assert_eq!(coordinates.byte_offset(), "aé€🦀".len());
        assert_eq!(coordinates.line(), 1);
        assert_eq!(coordinates.column(), 5);
    }

    #[test]
    fn newline_advances_line_and_resets_column() {
        let mut coordinates = SourceCoordinates::default();

        coordinates.advance("abc\nxy");

        assert_eq!(coordinates.byte_offset(), 6);
        assert_eq!(coordinates.line(), 2);
        assert_eq!(coordinates.column(), 3);
    }

    #[test]
    fn crlf_preserves_existing_scanner_semantics() {
        let mut coordinates = SourceCoordinates::default();

        coordinates.advance("FIRST=ok\r\nSECOND=");

        assert_eq!(coordinates.byte_offset(), "FIRST=ok\r\nSECOND=".len());
        assert_eq!(coordinates.line(), 2);
        assert_eq!(coordinates.column(), 8);
    }

    #[test]
    fn empty_segments_are_no_ops() {
        let mut coordinates = SourceCoordinates::default();

        coordinates.advance("");

        assert_eq!(coordinates, SourceCoordinates::default());
    }

    #[test]
    fn segmentation_does_not_change_coordinates() {
        let source = "aé\r\n€🦀\nxyz";

        let mut whole = SourceCoordinates::default();
        whole.advance(source);

        for split in 0..=source.len() {
            if !source.is_char_boundary(split) {
                continue;
            }

            let mut segmented = SourceCoordinates::default();
            segmented.advance(&source[..split]);
            segmented.advance(&source[split..]);

            assert_eq!(segmented, whole, "split at byte {split}");
        }
    }

    #[test]
    fn arbitrary_byte_partitioning_does_not_change_coordinates() {
        let source = "aé\r\n€🦀\nxyz";
        let bytes = source.as_bytes();

        let mut expected = SourceCoordinates::default();
        expected.advance(source);

        for split in 0..=bytes.len() {
            let mut transport = Utf8Transport::default();
            let mut coordinates = SourceCoordinates::default();

            transport
                .push(&bytes[..split], |text| coordinates.advance(text))
                .unwrap();
            transport
                .push(&bytes[split..], |text| coordinates.advance(text))
                .unwrap();
            transport.finish().unwrap();

            assert_eq!(coordinates, expected, "partition at byte {split}");
        }
    }

    #[test]
    fn single_byte_transport_preserves_coordinates() {
        let source = "aé\r\n€🦀\nxyz";

        let mut expected = SourceCoordinates::default();
        expected.advance(source);

        let mut transport = Utf8Transport::default();
        let mut coordinates = SourceCoordinates::default();

        for byte in source.as_bytes() {
            transport
                .push(std::slice::from_ref(byte), |text| coordinates.advance(text))
                .unwrap();
        }

        transport.finish().unwrap();

        assert_eq!(coordinates, expected);
    }
}
