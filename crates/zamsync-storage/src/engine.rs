use std::path::Path;
use zamsync_core::{Event, SequenceNumber, ZamResult, ZamError};
use crate::wal::{WalWriter, WalScanner};
use crate::state::StateStore;

pub struct ZamEngine<S: StateStore> {
    writer: WalWriter,
    state: S,
}

impl<S: StateStore> ZamEngine<S> {
    /// Initialise the engine. 
    /// If the WAL already exists, it will be scanned to recover the current state.
    pub fn open(path: impl AsRef<Path>, mut state: S) -> ZamResult<Self> {
        let (last_seq, recover_pos) = WalScanner::recover(&path)?;
        
        // If there is data to recover, replay it into the state projection
        if let Some(_) = last_seq {
            let scanner = WalScanner::open(&path)?;
            let mut it = scanner.scan();
            while let Some(record) = it.next_record()? {
                // Deserialize event using rkyv
                let event: Event = rkyv::from_bytes(&record.payload)
                    .map_err(|e| ZamError::Serialization(format!("Failed to deserialize event: {}", e)))?;
                
                state.apply_event(record.seq, &event);
            }
        }

        let next_seq = last_seq.map(|s| s.next()).unwrap_or(SequenceNumber::ZERO);
        let writer = WalWriter::open(path, next_seq)?;

        Ok(Self { writer, state })
    }

    /// Submit a new event to the system.
    /// Returns the sequence number assigned to the event.
    pub fn submit(&mut self, event: Event) -> ZamResult<SequenceNumber> {
        // 1. Serialize
        let bytes = rkyv::to_bytes::<_, 1024>(&event)
            .map_err(|e| ZamError::Serialization(format!("Failed to serialize event: {}", e)))?;
        
        // 2. Append to WAL (Persistence)
        let seq = self.writer.append(&bytes)?;
        
        // 3. Apply to state (Projection)
        self.state.apply_event(seq, &event);
        
        Ok(seq)
    }

    pub fn state(&self) -> &S {
        &self.state
    }

    pub fn sync(&mut self) -> ZamResult<()> {
        self.writer.sync().map_err(Into::into)
    }
}
