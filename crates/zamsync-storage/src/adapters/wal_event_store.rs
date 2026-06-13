use crate::wal::{WalScanner, WalWriter};
use std::path::{Path, PathBuf};
use zamsync_core::ports::EventStore;
use zamsync_core::{Event, SequenceNumber, ZamError, ZamResult};

pub struct WalEventStore {
    path: PathBuf,
    writer: WalWriter,
}

impl WalEventStore {
    pub fn open(path: impl AsRef<Path>) -> ZamResult<Self> {
        let (last_seq, end_pos) = WalScanner::recover(&path)?;

        // Truncate any bytes left past the last valid record. Without this, a
        // crash mid-write leaves a partial record in the file. WalWriter opens in
        // APPEND mode, so new records land after the garbage -- and the next scan
        // stops at the garbage, making those new records permanently invisible.
        if path.as_ref().exists() {
            let actual_len = std::fs::metadata(path.as_ref())?.len();
            if actual_len > end_pos {
                std::fs::OpenOptions::new()
                    .write(true)
                    .open(path.as_ref())?
                    .set_len(end_pos)?;
            }
        }

        let next_seq = last_seq.map(|s| s.next()).unwrap_or(SequenceNumber::ZERO);
        let writer = WalWriter::open(&path, next_seq)?;
        Ok(Self {
            path: path.as_ref().to_path_buf(),
            writer,
        })
    }
}

impl EventStore for WalEventStore {
    fn next_seq(&self) -> SequenceNumber {
        self.writer.next_seq()
    }

    fn append(&mut self, event: &Event) -> ZamResult<SequenceNumber> {
        let bytes =
            rkyv::to_bytes::<_, 1024>(event).map_err(|e| ZamError::Serialization(e.to_string()))?;
        self.writer.append(&bytes)
    }

    fn scan(&self) -> ZamResult<Box<dyn Iterator<Item = ZamResult<Event>>>> {
        if !self.path.exists() {
            return Ok(Box::new(std::iter::empty()));
        }
        let scanner = WalScanner::open(&self.path)?;
        let iter = scanner.scan().map(|res| -> ZamResult<Event> {
            let record = res?;
            rkyv::from_bytes::<Event>(&record.payload)
                .map_err(|e| ZamError::Serialization(format!("{}", e)))
        });
        Ok(Box::new(iter))
    }

    fn sync(&mut self) -> ZamResult<()> {
        self.writer.sync().map_err(Into::into)
    }
}
