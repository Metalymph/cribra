//! Incremental UTF-8 transport validation.
//!
//! This module validates arbitrarily partitioned byte input while retaining
//! only the incomplete trailing bytes required to reconstruct a UTF-8 scalar.
//! It performs no I/O and owns no source-sized buffering.

/// Maximum number of trailing bytes required to reconstruct a UTF-8 scalar
/// split across transport boundaries.
const MAX_UTF8_CARRY: usize = 3;

/// Incremental UTF-8 transport error.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) enum Utf8TransportError {
    /// The input contains a definitively malformed UTF-8 sequence.
    Malformed,
    /// The source ended while a UTF-8 scalar was still incomplete.
    Incomplete,
}

/// Incremental UTF-8 transport state.
///
/// The transport retains at most three bytes: a valid but incomplete prefix of
/// one UTF-8 scalar split across transport calls.
#[derive(Debug, Default)]
pub(crate) struct Utf8Transport {
    carry: [u8; MAX_UTF8_CARRY],
    carry_len: u8,
}

impl Utf8Transport {
    /// Accepts the next arbitrary byte fragment and visits complete UTF-8
    /// segments as soon as they become available.
    ///
    /// Empty fragments are transport no-ops. Complete valid UTF-8 is visited
    /// immediately. A valid but incomplete trailing scalar prefix is retained
    /// until a later fragment completes it.
    pub(crate) fn push<F>(&mut self, bytes: &[u8], mut visit: F) -> Result<(), Utf8TransportError>
    where
        F: FnMut(&str),
    {
        if bytes.is_empty() {
            return Ok(());
        }

        if self.carry_len == 0 {
            return self.accept_without_carry(bytes, &mut visit);
        }

        self.resolve_carry(bytes, &mut visit)
    }

    /// Verifies that the logical source ended on a UTF-8 scalar boundary.
    pub(crate) fn finish(&self) -> Result<(), Utf8TransportError> {
        if self.carry_len == 0 {
            Ok(())
        } else {
            Err(Utf8TransportError::Incomplete)
        }
    }

    fn accept_without_carry<F>(
        &mut self,
        bytes: &[u8],
        visit: &mut F,
    ) -> Result<(), Utf8TransportError>
    where
        F: FnMut(&str),
    {
        match std::str::from_utf8(bytes) {
            Ok(text) => {
                if !text.is_empty() {
                    visit(text);
                }
                Ok(())
            }
            Err(error) if error.error_len().is_some() => Err(Utf8TransportError::Malformed),
            Err(error) => {
                let valid_up_to = error.valid_up_to();

                if valid_up_to != 0 {
                    let text = std::str::from_utf8(&bytes[..valid_up_to])
                        .map_err(|_| Utf8TransportError::Malformed)?;
                    visit(text);
                }

                self.store_incomplete_tail(&bytes[valid_up_to..])
            }
        }
    }

    fn resolve_carry<F>(&mut self, bytes: &[u8], visit: &mut F) -> Result<(), Utf8TransportError>
    where
        F: FnMut(&str),
    {
        let carry_len = usize::from(self.carry_len);
        let scalar_width =
            Self::scalar_width(self.carry[0]).ok_or(Utf8TransportError::Malformed)?;

        debug_assert!(scalar_width > carry_len);

        let missing = scalar_width - carry_len;
        let taken = bytes.len().min(missing);

        let mut scalar = [0_u8; 4];
        scalar[..carry_len].copy_from_slice(&self.carry[..carry_len]);
        scalar[carry_len..carry_len + taken].copy_from_slice(&bytes[..taken]);

        let scalar_len = carry_len + taken;

        match std::str::from_utf8(&scalar[..scalar_len]) {
            Ok(text) => {
                // A carried scalar can become valid only when its complete
                // encoded width has been reconstructed.
                if scalar_len != scalar_width {
                    return Err(Utf8TransportError::Malformed);
                }

                visit(text);
                self.clear_carry();

                self.accept_without_carry(&bytes[taken..], visit)
            }
            Err(error) if error.error_len().is_some() => Err(Utf8TransportError::Malformed),
            Err(_) if taken == bytes.len() => self.store_incomplete_tail(&scalar[..scalar_len]),
            Err(_) => Err(Utf8TransportError::Malformed),
        }
    }

    fn scalar_width(first: u8) -> Option<usize> {
        match first {
            0x00..=0x7F => Some(1),
            0xC2..=0xDF => Some(2),
            0xE0..=0xEF => Some(3),
            0xF0..=0xF4 => Some(4),
            _ => None,
        }
    }

    fn store_incomplete_tail(&mut self, tail: &[u8]) -> Result<(), Utf8TransportError> {
        if tail.is_empty() || tail.len() > MAX_UTF8_CARRY {
            return Err(Utf8TransportError::Malformed);
        }

        self.carry = [0; MAX_UTF8_CARRY];
        self.carry[..tail.len()].copy_from_slice(tail);
        self.carry_len = tail.len() as u8;

        Ok(())
    }

    fn clear_carry(&mut self) {
        self.carry = [0; MAX_UTF8_CARRY];
        self.carry_len = 0;
    }

    #[cfg(test)]
    fn carry_len(&self) -> usize {
        usize::from(self.carry_len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complete_utf8_requires_no_carry() {
        let mut transport = Utf8Transport::default();

        transport.push("ascii € 🦀".as_bytes(), |_| {}).unwrap();

        assert_eq!(transport.carry_len(), 0);
        assert_eq!(transport.finish(), Ok(()));
    }

    #[test]
    fn empty_fragments_are_no_ops() {
        let mut transport = Utf8Transport::default();

        transport.push(&[], |_| {}).unwrap();
        transport.push(&[], |_| {}).unwrap();

        assert_eq!(transport.carry_len(), 0);
        assert_eq!(transport.finish(), Ok(()));
    }

    #[test]
    fn split_multibyte_scalar_is_reconstructed() {
        let bytes = "🦀".as_bytes();
        let mut transport = Utf8Transport::default();

        transport.push(&bytes[..1], |_| {}).unwrap();
        assert_eq!(transport.carry_len(), 1);

        transport.push(&bytes[1..2], |_| {}).unwrap();
        assert_eq!(transport.carry_len(), 2);

        transport.push(&bytes[2..3], |_| {}).unwrap();
        assert_eq!(transport.carry_len(), 3);

        transport.push(&bytes[3..], |_| {}).unwrap();

        assert_eq!(transport.carry_len(), 0);
        assert_eq!(transport.finish(), Ok(()));
    }

    #[test]
    fn single_byte_partition_accepts_valid_utf8() {
        let source = "aé€🦀z";
        let mut transport = Utf8Transport::default();

        for byte in source.as_bytes() {
            transport.push(std::slice::from_ref(byte), |_| {}).unwrap();
            assert!(transport.carry_len() <= MAX_UTF8_CARRY);
        }

        assert_eq!(transport.finish(), Ok(()));
    }

    #[test]
    fn malformed_utf8_is_rejected() {
        let mut transport = Utf8Transport::default();

        assert_eq!(
            transport.push(&[0xC3, 0x28], |_| {}),
            Err(Utf8TransportError::Malformed)
        );
    }

    #[test]
    fn malformed_utf8_crossing_transport_boundary_is_rejected() {
        let mut transport = Utf8Transport::default();

        transport.push(&[0xE2], |_| {}).unwrap();

        assert_eq!(
            transport.push(&[0x28], |_| {}),
            Err(Utf8TransportError::Malformed)
        );
    }

    #[test]
    fn incomplete_utf8_is_rejected_only_at_end_of_source() {
        let mut transport = Utf8Transport::default();

        transport.push(&[0xF0, 0x9F, 0xA6], |_| {}).unwrap();

        assert_eq!(transport.carry_len(), 3);
        assert_eq!(transport.finish(), Err(Utf8TransportError::Incomplete));
    }

    #[test]
    fn empty_fragment_does_not_finalize_pending_scalar() {
        let mut transport = Utf8Transport::default();

        transport.push(&[0xE2], |_| {}).unwrap();
        transport.push(&[], |_| {}).unwrap();

        assert_eq!(transport.carry_len(), 1);

        transport.push(&[0x82, 0xAC], |_| {}).unwrap();

        assert_eq!(transport.finish(), Ok(()));
    }

    #[test]
    fn complete_input_is_delivered() {
        let mut transport = Utf8Transport::default();
        let mut output = String::new();

        transport
            .push("hello 🦀".as_bytes(), |text| output.push_str(text))
            .unwrap();

        assert_eq!(output, "hello 🦀");
        assert_eq!(transport.finish(), Ok(()));
    }

    #[test]
    fn split_scalar_is_delivered_exactly_once() {
        let bytes = "🦀".as_bytes();
        let mut transport = Utf8Transport::default();
        let mut output = String::new();

        for byte in bytes {
            transport
                .push(std::slice::from_ref(byte), |text| output.push_str(text))
                .unwrap();
        }

        assert_eq!(output, "🦀");
        assert_eq!(transport.finish(), Ok(()));
    }

    #[test]
    fn reconstructed_scalar_precedes_same_fragment_remainder() {
        let euro = "€".as_bytes();
        let mut transport = Utf8Transport::default();
        let mut output = String::new();

        transport
            .push(&euro[..1], |text| output.push_str(text))
            .unwrap();

        let mut next = Vec::from(&euro[1..]);
        next.extend_from_slice(b"hello");

        transport.push(&next, |text| output.push_str(text)).unwrap();

        assert_eq!(output, "€hello");
    }

    #[test]
    fn delivery_is_invariant_across_every_single_split() {
        let source = "aé€🦀z";
        let bytes = source.as_bytes();

        for split in 0..=bytes.len() {
            let mut transport = Utf8Transport::default();
            let mut output = String::new();

            transport
                .push(&bytes[..split], |text| output.push_str(text))
                .unwrap();

            transport
                .push(&bytes[split..], |text| output.push_str(text))
                .unwrap();

            transport.finish().unwrap();

            assert_eq!(output, source, "partition at byte {split}");
        }
    }

    #[test]
    fn single_byte_delivery_reconstructs_original_source() {
        let source = "aé€🦀z";
        let mut transport = Utf8Transport::default();
        let mut output = String::new();

        for byte in source.as_bytes() {
            transport
                .push(std::slice::from_ref(byte), |text| output.push_str(text))
                .unwrap();
        }

        transport.finish().unwrap();

        assert_eq!(output, source);
    }

    #[test]
    fn malformed_utf8_failure_is_invariant_across_every_split() {
        let malformed_cases: &[&[u8]] = &[
            &[0xC3, 0x28],
            &[0xE2, 0x28, 0xA1],
            &[0xF0, 0x28, 0x8C, 0xBC],
            &[0xED, 0xA0, 0x80],       // UTF-16 surrogate encoding
            &[0xF4, 0x90, 0x80, 0x80], // above U+10FFFF
            &[0x80],                   // stray continuation byte
            &[0xC0, 0xAF],             // overlong encoding
        ];

        for bytes in malformed_cases {
            for split in 0..=bytes.len() {
                let mut transport = Utf8Transport::default();

                let first = transport.push(&bytes[..split], |_| {});
                let second = if first.is_ok() {
                    transport.push(&bytes[split..], |_| {})
                } else {
                    first
                };

                assert_eq!(
                    second,
                    Err(Utf8TransportError::Malformed),
                    "case {bytes:02X?}, split at byte {split}"
                );
            }
        }
    }

    #[test]
    fn incomplete_utf8_end_of_source_is_invariant_across_every_split() {
        let incomplete_cases: &[&[u8]] = &[
            &[0xC2],
            &[0xE2],
            &[0xE2, 0x82],
            &[0xF0],
            &[0xF0, 0x9F],
            &[0xF0, 0x9F, 0xA6],
        ];

        for bytes in incomplete_cases {
            for split in 0..=bytes.len() {
                let mut transport = Utf8Transport::default();

                transport.push(&bytes[..split], |_| {}).unwrap();
                transport.push(&bytes[split..], |_| {}).unwrap();

                assert_eq!(
                    transport.finish(),
                    Err(Utf8TransportError::Incomplete),
                    "case {bytes:02X?}, split at byte {split}"
                );
            }
        }
    }

    #[test]
    fn empty_fragments_do_not_change_pending_coordinates_or_completion() {
        use crate::source_coordinates::SourceCoordinates;

        let crab = "🦀".as_bytes();
        let mut transport = Utf8Transport::default();
        let mut coordinates = SourceCoordinates::default();

        transport
            .push(&crab[..2], |text| coordinates.advance(text))
            .unwrap();

        let pending_coordinates = coordinates;

        for _ in 0..8 {
            transport
                .push(&[], |text| coordinates.advance(text))
                .unwrap();
            assert_eq!(coordinates, pending_coordinates);
            assert_eq!(transport.finish(), Err(Utf8TransportError::Incomplete));
        }

        transport
            .push(&crab[2..], |text| coordinates.advance(text))
            .unwrap();

        assert_eq!(transport.finish(), Ok(()));
        assert_eq!(coordinates.byte_offset(), crab.len());
        assert_eq!(coordinates.line(), 1);
        assert_eq!(coordinates.column(), 2);
    }
}
