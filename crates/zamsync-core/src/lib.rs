use std::fmt;
use rkyv::{Archive, Deserialize, Serialize};

/// Hybrid Logical Clock (HLC) for causal ordering.
#[derive(Archive, Deserialize, Serialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[archive(check_bytes)]
pub struct Hlc {
    /// Physical time (e.g., milliseconds since epoch).
    pub physical: u64,
    /// Logical counter for events within the same physical tick.
    pub logical: u32,
}

impl Hlc {
    pub fn new(physical: u64, logical: u32) -> Self {
        Self { physical, logical }
    }

    /// Update HLC with local physical time.
    pub fn tick(&mut self, now_ms: u64) {
        if now_ms > self.physical {
            self.physical = now_ms;
            self.logical = 0;
        } else {
            self.logical += 1;
        }
    }

    /// Sync HLC with a remote timestamp.
    pub fn sync(&mut self, now_ms: u64, remote: &Hlc) {
        let max_phys = now_ms.max(self.physical).max(remote.physical);
        
        if max_phys == self.physical && max_phys == remote.physical {
            self.logical = self.logical.max(remote.logical) + 1;
        } else if max_phys == self.physical {
            self.logical += 1;
        } else if max_phys == remote.physical {
            self.physical = remote.physical;
            self.logical = remote.logical + 1;
        } else {
            self.physical = max_phys;
            self.logical = 0;
        }
    }
}

/// Magic number for ZamSync WAL files: "ZAM!" in ASCII
pub const WAL_MAGIC: [u8; 4] = [0x5A, 0x41, 0x4D, 0x21];
pub const WAL_VERSION: u8 = 1;

pub mod sync;
pub use sync::*;

/// Unique identifier for a node in the ZamSync network.
#[derive(Archive, Deserialize, Serialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[archive(check_bytes)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NodeId(pub u32);

/// Monotonically increasing sequence number for events.
#[derive(Archive, Deserialize, Serialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[archive(check_bytes)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SequenceNumber(pub u64);

impl SequenceNumber {
    pub const ZERO: Self = Self(0);
    
    pub fn next(&self) -> Self {
        Self(self.0 + 1)
    }
}

impl fmt::Display for SequenceNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Generic Event structure for the ZamSync infrastructure.
/// It contains no domain-specific knowledge.
#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive(check_bytes)]
pub struct Event {
    /// Identity of the node that created this event.
    pub origin_node: NodeId,
    /// Monotonic sequence number from the origin node.
    pub seq: SequenceNumber,
    /// Hybrid Logical Clock timestamp for total ordering.
    pub hlc: Hlc,
    /// Application-defined type or namespace.
    pub event_type: u32,
    /// Opaque binary data.
    pub payload: Vec<u8>,
}

/// Common error types for the ZamSync system.
#[derive(Debug, thiserror::Error)]
pub enum ZamError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Data corruption detected: {0}")]
    Corruption(String),
    
    #[error("Serialization error: {0}")]
    Serialization(String),
    
    #[error("Protocol error: {0}")]
    Protocol(String),
    
    #[error("Invalid configuration: {0}")]
    Config(String),

    #[error("Storage engine error: {0}")]
    Storage(String),
}

pub type ZamResult<T> = Result<T, ZamError>;

/// Represents a validated chunk of data for transport.
#[derive(Debug, Clone)]
pub struct Chunk {
    pub seq: SequenceNumber,
    pub data: Vec<u8>,
    pub crc: u32,
}
