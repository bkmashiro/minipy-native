//! Immutable, bounded storage for Quote/v1 records.

use crate::error::TaskError;
use std::mem::size_of;
use std::sync::{Arc, Weak};

/// Maximum number of Quote rows accepted by the prototype ABI.
pub const MAX_QUOTES: usize = 1_000_000;
pub const MAX_RECORD_BYTES: usize = MAX_QUOTES * 16;
const QUOTE_STRIDE: usize = 16;

/// Quote/v1 in-memory record. Its C layout matches the fixed 16-byte ABI.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Quote {
    pub unit_price_cents: i64,
    pub quantity: i64,
}

/// Observes the small owner header, never pins the separately boxed row payload.
pub type QuoteWeak = Weak<Box<[Quote]>>;

/// Immutable, shared ownership of a bounded array of Quote records.
#[derive(Clone, Debug)]
pub struct QuoteBuffer {
    rows: Arc<Box<[Quote]>>,
}

impl QuoteBuffer {
    /// Freeze rows into an immutable owner, rejecting inputs beyond the row bound.
    /// Exact-capacity Vec payloads are moved; spare capacity may shrink here.
    pub fn from_quotes(quotes: Vec<Quote>) -> Result<Self, TaskError> {
        if quotes.len() > MAX_QUOTES {
            return Err(TaskError::Budget);
        }
        quotes
            .len()
            .checked_mul(size_of::<Quote>())
            .ok_or(TaskError::Overflow)?;
        Ok(Self {
            rows: Arc::new(quotes.into_boxed_slice()),
        })
    }

    /// Import Quote/v1 little-endian records into a trusted immutable owner.
    /// Length must describe whole records and stay within the configured row bound.
    pub fn import_bytes(bytes: &[u8]) -> Result<Self, TaskError> {
        if !bytes.len().is_multiple_of(QUOTE_STRIDE) {
            return Err(TaskError::Buffer);
        }
        let count = bytes.len() / QUOTE_STRIDE;
        if count > MAX_QUOTES {
            return Err(TaskError::Budget);
        }
        let allocation_bytes = count
            .checked_mul(size_of::<Quote>())
            .ok_or(TaskError::Overflow)?;
        if allocation_bytes != bytes.len() {
            return Err(TaskError::Buffer);
        }

        let mut rows = Vec::with_capacity(count);
        for record in bytes.chunks_exact(QUOTE_STRIDE) {
            let price = i64::from_le_bytes(record[0..8].try_into().map_err(|_| TaskError::Buffer)?);
            let quantity =
                i64::from_le_bytes(record[8..16].try_into().map_err(|_| TaskError::Buffer)?);
            rows.push(Quote {
                unit_price_cents: price,
                quantity,
            });
        }
        Self::from_quotes(rows)
    }

    /// Borrow rows only while this immutable owner remains alive.
    #[inline]
    pub fn rows(&self) -> &[Quote] {
        &self.rows
    }

    /// Number of bytes in the exact frozen row allocation.
    #[inline]
    pub fn bytes(&self) -> usize {
        self.rows
            .len()
            .checked_mul(size_of::<Quote>())
            .expect("bounded QuoteBuffer byte size")
    }

    /// Observe owner lifetime without extending it.
    #[inline]
    pub fn downgrade(&self) -> QuoteWeak {
        Arc::downgrade(&self.rows)
    }
}

/// Aligned, immutable fixed-width rows. Offsets and stride are bound at preparation.
#[cfg(feature = "tool-json")]
#[derive(Clone, Debug)]
pub struct RecordBuffer {
    schema: crate::tool::RecordSchema,
    words: Arc<Box<[i64]>>,
}

#[cfg(feature = "tool-json")]
impl RecordBuffer {
    pub fn import_bytes(
        schema: crate::tool::RecordSchema,
        bytes: &[u8],
    ) -> Result<Self, TaskError> {
        schema.validate().map_err(|_| TaskError::Buffer)?;
        if bytes.len() > MAX_RECORD_BYTES || bytes.len() / schema.stride > MAX_QUOTES {
            return Err(TaskError::Budget);
        }
        if !bytes.len().is_multiple_of(schema.stride) {
            return Err(TaskError::Buffer);
        }
        let mut words = Vec::with_capacity(bytes.len() / 8);
        for cell in bytes.chunks_exact(8) {
            words.push(i64::from_le_bytes(
                cell.try_into().map_err(|_| TaskError::Buffer)?,
            ));
        }
        let bool_offsets: Vec<_> = schema
            .fields
            .iter()
            .filter(|f| f.ty == crate::ir::Type::Bool)
            .map(|f| f.offset / 8)
            .collect();
        if !bool_offsets.is_empty() {
            for row in words.chunks_exact(schema.stride / 8) {
                if bool_offsets
                    .iter()
                    .any(|&offset| ![0, 1].contains(&row[offset]))
                {
                    return Err(TaskError::Buffer);
                }
            }
        }
        Ok(Self {
            schema,
            words: Arc::new(words.into_boxed_slice()),
        })
    }

    pub fn schema(&self) -> &crate::tool::RecordSchema {
        &self.schema
    }
    pub fn bytes(&self) -> usize {
        self.words.len() * 8
    }
    pub fn len(&self) -> usize {
        self.bytes() / self.schema.stride
    }
    pub fn is_empty(&self) -> bool {
        self.words.is_empty()
    }
    pub fn words(&self) -> &[i64] {
        &self.words
    }
    pub fn downgrade(&self) -> Weak<Box<[i64]>> {
        Arc::downgrade(&self.words)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::align_of;
    use std::ptr::addr_of;
    #[test]
    fn exact_capacity_freeze_moves_the_payload_and_uses_a_thin_owner() {
        let rows = vec![
            Quote {
                unit_price_cents: 3,
                quantity: 2
            };
            4
        ];
        assert_eq!(rows.len(), rows.capacity());
        let ptr = rows.as_ptr();
        let owner = QuoteBuffer::from_quotes(rows).unwrap();
        assert_eq!(owner.rows().as_ptr(), ptr);
        assert_eq!(size_of::<QuoteBuffer>(), size_of::<usize>());
        let weak: QuoteWeak = owner.downgrade();
        drop(owner);
        assert!(weak.upgrade().is_none());
    }
    #[test]
    fn quote_v1_layout_and_offsets_are_stable() {
        assert_eq!(size_of::<Quote>(), QUOTE_STRIDE);
        assert_eq!(align_of::<Quote>(), align_of::<i64>());
        let quote = Quote {
            unit_price_cents: 1,
            quantity: 2,
        };
        let base = &quote as *const Quote as usize;
        assert_eq!(addr_of!(quote.unit_price_cents) as usize - base, 0);
        assert_eq!(addr_of!(quote.quantity) as usize - base, 8);
    }

    #[test]
    fn immutable_owner_clones_and_weak_observation_share_one_allocation() {
        let owner = QuoteBuffer::from_quotes(vec![
            Quote {
                unit_price_cents: 7,
                quantity: -3,
            },
            Quote {
                unit_price_cents: i64::MIN,
                quantity: i64::MAX,
            },
        ])
        .unwrap();
        let weak = owner.downgrade();
        let clone = owner.clone();
        assert_eq!(owner.rows()[0].unit_price_cents, 7);
        assert_eq!(owner.bytes(), 32);
        assert!(Arc::ptr_eq(&weak.upgrade().unwrap(), &clone.rows));
        drop(owner);
        assert!(weak.upgrade().is_some());
        drop(clone);
        assert!(weak.upgrade().is_none());
    }

    #[test]
    fn import_decodes_little_endian_and_preserves_extreme_patterns() {
        let values = [i64::MIN, i64::MAX, -1, 0];
        let bytes: Vec<u8> = values.into_iter().flat_map(i64::to_le_bytes).collect();
        let owner = QuoteBuffer::import_bytes(&bytes).unwrap();
        assert_eq!(
            owner.rows(),
            &[
                Quote {
                    unit_price_cents: i64::MIN,
                    quantity: i64::MAX
                },
                Quote {
                    unit_price_cents: -1,
                    quantity: 0
                },
            ]
        );
        assert_eq!(owner.bytes(), 32);
    }

    #[test]
    fn empty_and_malformed_buffers_are_handled_without_raw_loads() {
        let empty = QuoteBuffer::import_bytes(&[]).unwrap();
        assert!(empty.rows().is_empty());
        assert_eq!(empty.bytes(), 0);
        for bytes in [&[0_u8][..], &[0_u8; 15][..], &[0_u8; 17][..]] {
            assert_eq!(
                QuoteBuffer::import_bytes(bytes).unwrap_err(),
                TaskError::Buffer
            );
        }
        let too_many = vec![0_u8; (MAX_QUOTES + 1) * QUOTE_STRIDE];
        assert_eq!(
            QuoteBuffer::import_bytes(&too_many).unwrap_err(),
            TaskError::Budget
        );
    }

    #[test]
    fn from_quotes_enforces_row_limit() {
        let too_many = vec![
            Quote {
                unit_price_cents: 0,
                quantity: 0
            };
            MAX_QUOTES + 1
        ];
        assert_eq!(
            QuoteBuffer::from_quotes(too_many).unwrap_err(),
            TaskError::Budget
        );
    }
}
