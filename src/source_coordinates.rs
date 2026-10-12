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

/// Counter that could not represent the position after validated text.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) enum CoordinateOverflow {
    ByteOffset,
    Line,
    Column,
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
    /// Synthetic positions for overflow and invariant tests, without huge input.
    #[cfg(test)]
    pub(crate) const fn from_parts(byte_offset: usize, line: usize, column: usize) -> Self {
        Self {
            byte_offset,
            line,
            column,
        }
    }

    /// Advances atomically over complete UTF-8 scalars using Scanner semantics.
    /// On overflow the original cursor is unchanged. Transport carry bytes must
    /// be supplied only when delivered as part of a complete validated callback.
    pub(crate) fn checked_advance(&mut self, text: &str) -> Result<(), CoordinateOverflow> {
        let mut next = *self;
        next.byte_offset = next
            .byte_offset
            .checked_add(text.len())
            .ok_or(CoordinateOverflow::ByteOffset)?;
        for character in text.chars() {
            if character == '\n' {
                next.line = next.line.checked_add(1).ok_or(CoordinateOverflow::Line)?;
                next.column = 1;
            } else {
                next.column = next
                    .column
                    .checked_add(1)
                    .ok_or(CoordinateOverflow::Column)?;
            }
        }
        *self = next;
        Ok(())
    }

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
    fn checked_transport_partitions_preserve_scalar_and_newline_semantics() {
        for source in ["", "ASCII", "aé€🦀", "a\rb\nc\r\nd", "aé\r\n€🦀\nxyz"] {
            let mut expected = SourceCoordinates::default();
            expected.advance(source);
            for size in [1, 2, 3, 7, source.len().max(1)] {
                let mut cursor = SourceCoordinates::default();
                let mut transport = Utf8Transport::default();
                transport
                    .push(b"", |text| cursor.checked_advance(text).unwrap())
                    .unwrap();
                for chunk in source.as_bytes().chunks(size) {
                    transport
                        .push(chunk, |text| cursor.checked_advance(text).unwrap())
                        .unwrap();
                }
                transport.finish().unwrap();
                assert_eq!(cursor, expected, "source={source:?}, size={size}");
                cursor.checked_advance("").unwrap();
                assert_eq!(cursor, expected);
            }
        }
    }

    #[test]
    fn checked_positions_match_whole_source_scanner() {
        let source = "aé\r\n€🦀\nxyz";
        for needle in ["a", "é", "\r", "\n", "€", "🦀", "xyz"] {
            let scanner = crate::Scanner::builder()
                .rule(crate::Rule::literal(
                    "position",
                    needle,
                    crate::Severity::High,
                ))
                .build()
                .unwrap();
            let result = scanner.scan([((), source)]);
            for (_, finding) in result.findings() {
                let mut cursor = SourceCoordinates::default();
                cursor
                    .checked_advance(&source[..finding.location().start()])
                    .unwrap();
                assert_eq!(cursor.byte_offset(), finding.location().start());
                assert_eq!(cursor.line(), finding.location().line());
                assert_eq!(cursor.column(), finding.location().column());
            }
        }
        let mut eof = SourceCoordinates::default();
        eof.checked_advance(source).unwrap();
        assert_eq!(
            (eof.byte_offset(), eof.line(), eof.column()),
            (source.len(), 3, 4)
        );
    }

    #[test]
    fn checked_counter_overflow_is_explicit_and_atomic() {
        for (original, text, error) in [
            (
                SourceCoordinates {
                    byte_offset: usize::MAX,
                    line: 1,
                    column: 1,
                },
                "a",
                CoordinateOverflow::ByteOffset,
            ),
            (
                SourceCoordinates {
                    byte_offset: usize::MAX - 1,
                    line: 1,
                    column: 1,
                },
                "é",
                CoordinateOverflow::ByteOffset,
            ),
            (
                SourceCoordinates {
                    byte_offset: 0,
                    line: usize::MAX,
                    column: 1,
                },
                "a\n",
                CoordinateOverflow::Line,
            ),
            (
                SourceCoordinates {
                    byte_offset: 0,
                    line: 1,
                    column: usize::MAX - 1,
                },
                "ab",
                CoordinateOverflow::Column,
            ),
        ] {
            let mut cursor = original;
            assert_eq!(cursor.checked_advance(text), Err(error));
            assert_eq!(cursor, original);
        }
    }

    #[test]
    fn checked_exact_boundaries_and_lf_reset_do_not_false_overflow() {
        let mut bytes = SourceCoordinates {
            byte_offset: usize::MAX - 4,
            line: 1,
            column: 1,
        };
        bytes.checked_advance("🦀").unwrap();
        assert_eq!(bytes.byte_offset(), usize::MAX);
        bytes.checked_advance("").unwrap();
        let mut line = SourceCoordinates {
            byte_offset: 0,
            line: usize::MAX - 1,
            column: usize::MAX,
        };
        line.checked_advance("\n").unwrap();
        assert_eq!((line.line(), line.column()), (usize::MAX, 1));
        line.checked_advance("a").unwrap();
        let mut column = SourceCoordinates {
            byte_offset: 0,
            line: 1,
            column: usize::MAX - 1,
        };
        column.checked_advance("\r").unwrap();
        assert_eq!(column.column(), usize::MAX);
        column.checked_advance("\n").unwrap();
        assert_eq!((column.line(), column.column()), (2, 1));
    }

    #[test]
    fn checked_cursor_does_not_count_incomplete_transport_carry() {
        let mut transport = Utf8Transport::default();
        let mut cursor = SourceCoordinates::default();
        transport
            .push(&[0xe2, 0x82], |text| cursor.checked_advance(text).unwrap())
            .unwrap();
        assert_eq!(cursor, SourceCoordinates::default());
        transport
            .push(&[0xac, b'\r'], |text| cursor.checked_advance(text).unwrap())
            .unwrap();
        assert_eq!(
            (cursor.byte_offset(), cursor.line(), cursor.column()),
            (4, 1, 3)
        );
        transport
            .push(b"\n", |text| cursor.checked_advance(text).unwrap())
            .unwrap();
        transport.finish().unwrap();
        assert_eq!(
            (cursor.byte_offset(), cursor.line(), cursor.column()),
            (5, 2, 1)
        );
        let mut incomplete = Utf8Transport::default();
        incomplete
            .push(&[0xf0, 0x9f], |text| cursor.checked_advance(text).unwrap())
            .unwrap();
        assert!(incomplete.finish().is_err());
        assert_eq!(cursor.byte_offset(), 5);
    }

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
