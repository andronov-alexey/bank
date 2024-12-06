use derive_more::Display;

#[derive(Debug, Display, Clone, Copy, PartialEq, Eq, Ord, PartialOrd, Hash)]
pub struct TransactionId(u64);

impl TransactionId {
    pub fn new(id: u64) -> TransactionId {
        Self(id)
    }

    pub fn value(&self) -> u64 {
        self.0
    }
}
