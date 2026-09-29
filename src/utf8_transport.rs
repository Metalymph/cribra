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
    /// Accepts the next arbitrary byte fragment.
    ///
    /// Empty fragments are transport no-ops. Complete valid UTF-8 is consumed
    /// immediately. A valid but incomplete trailing scalar prefix is retained
    /// until a later fragment completes it.
    pub(crate) fn push(&mut self, bytes: &[u8]) -> Result<(), Utf8TransportError> {
        if bytes.is_empty() {
            return Ok(());
        }

        if self.carry_len == 0 {
            return self.accept_without_carry(bytes);
        }

        self.resolve_carry(bytes)
    }

    /// Verifies that the logical source ended on a UTF-8 scalar boundary.
    pub(crate) fn finish(&self) -> Result<(), Utf8TransportError> {
        if self.carry_len == 0 {
            Ok(())
        } else {
            Err(Utf8TransportError::Incomplete)
        }
    }

    fn accept_without_carry(&mut self, bytes: &[u8]) -> Result<(), Utf8TransportError> {
        match std::str::from_utf8(bytes) {
            Ok(_) => Ok(()),
            Err(error) => {
                if error.error_len().is_some() {
                    return Err(Utf8TransportError::Malformed);
                }

                self.store_incomplete_tail(&bytes[error.valid_up_to()..])
            }
        }
    }

    fn resolve_carry(&mut self, bytes: &[u8]) -> Result<(), Utf8TransportError> {
        // One UTF-8 scalar is at most four bytes. Existing carry is at most
        // three, so only enough bytes to resolve that scalar need temporary
        // stack storage.
        let carry_len = usize::from(self.carry_len);
        let needed = 4 - carry_len;
        let taken = bytes.len().min(needed);

        let mut boundary = [0_u8; 4];
        boundary[..carry_len].copy_from_slice(&self.carry[..carry_len]);
        boundary[carry_len..carry_len + taken].copy_from_slice(&bytes[..taken]);

        let boundary_len = carry_len + taken;

        match std::str::from_utf8(&boundary[..boundary_len]) {
            Ok(_) => {
                self.clear_carry();
                self.accept_without_carry(&bytes[taken..])
            }
            Err(error) if error.error_len().is_some() => Err(Utf8TransportError::Malformed),
            Err(_) if taken == bytes.len() => {
                self.store_incomplete_tail(&boundary[..boundary_len])
            }
            Err(_) => Err(Utf8TransportError::Malformed),
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

        transport.push("ascii € 🦀".as_bytes()).unwrap();

        assert_eq!(transport.carry_len(), 0);
        assert_eq!(transport.finish(), Ok(()));
    }

    #[test]
    fn empty_fragments_are_no_ops() {
        let mut transport = Utf8Transport::default();

        transport.push(&[]).unwrap();
        transport.push(&[]).unwrap();

        assert_eq!(transport.carry_len(), 0);
        assert_eq!(transport.finish(), Ok(()));
    }

    #[test]
    fn split_multibyte_scalar_is_reconstructed() {
        let bytes = "🦀".as_bytes();
        let mut transport = Utf8Transport::default();

        transport.push(&bytes[..1]).unwrap();
        assert_eq!(transport.carry_len(), 1);

        transport.push(&bytes[1..2]).unwrap();
        assert_eq!(transport.carry_len(), 2);

        transport.push(&bytes[2..3]).unwrap();
        assert_eq!(transport.carry_len(), 3);

        transport.push(&bytes[3..]).unwrap();

        assert_eq!(transport.carry_len(), 0);
        assert_eq!(transport.finish(), Ok(()));
    }

    #[test]
    fn single_byte_partition_accepts_valid_utf8() {
        let source = "aé€🦀z";
        let mut transport = Utf8Transport::default();

        for byte in source.as_bytes() {
            transport.push(std::slice::from_ref(byte)).unwrap();
            assert!(transport.carry_len() <= MAX_UTF8_CARRY);
        }

        assert_eq!(transport.finish(), Ok(()));
    }

    #[test]
    fn malformed_utf8_is_rejected() {
        let mut transport = Utf8Transport::default();

        assert_eq!(
            transport.push(&[0xC3, 0x28]),
            Err(Utf8TransportError::Malformed)
        );
    }

    #[test]
    fn malformed_utf8_crossing_transport_boundary_is_rejected() {
        let mut transport = Utf8Transport::default();

        transport.push(&[0xE2]).unwrap();

        assert_eq!(
            transport.push(&[0x28]),
            Err(Utf8TransportError::Malformed)
        );
    }

    #[test]
    fn incomplete_utf8_is_rejected_only_at_end_of_source() {
        let mut transport = Utf8Transport::default();

        transport.push(&[0xF0, 0x9F, 0xA6]).unwrap();

        assert_eq!(transport.carry_len(), 3);
        assert_eq!(transport.finish(), Err(Utf8TransportError::Incomplete));
    }

    #[test]
    fn empty_fragment_does_not_finalize_pending_scalar() {
        let mut transport = Utf8Transport::default();

        transport.push(&[0xE2]).unwrap();
        transport.push(&[]).unwrap();

        assert_eq!(transport.carry_len(), 1);

        transport.push(&[0x82, 0xAC]).unwrap();

        assert_eq!(transport.finish(), Ok(()));
    }
}