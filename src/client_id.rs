use derive_more::Display;

#[derive(Debug, Display, Clone, Copy, PartialEq, Eq, Ord, PartialOrd, Hash)]
pub struct ClientId(u16);

impl ClientId {
    pub fn new(id: u16) -> ClientId {
        Self(id)
    }

    pub fn value(&self) -> u16 {
        self.0
    }
}
