use crate::wal::{WalScanner, WalWriter};
use std::collections::HashMap;
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

impl WalEventStore {
    /// Rewrites the WAL, dropping all events whose `seq <= frontier[origin_node]`.
    /// Events at or below the frontier have been confirmed by all known peers and
    /// are safe to drop. Original WAL seq numbers are preserved (not renumbered)
    /// so that the next local-event seq continues correctly after reopening.
    ///
    /// If ALL events are dropped, a zero-payload tombstone is written at the
    /// last seq so that `WalWriter.current_seq` continues from the right position
    /// instead of resetting to zero.
    ///
    /// Returns the number of records dropped. Returns 0 if nothing was dropped.
    pub fn compact(&mut self, frontier: &HashMap<u32, SequenceNumber>) -> ZamResult<usize> {
        if !self.path.exists() || frontier.is_empty() {
            return Ok(0);
        }

        self.writer.sync()?;

        let mut kept: Vec<(SequenceNumber, Vec<u8>)> = Vec::new();
        let mut dropped = 0usize;
        let mut last_seen_seq: Option<SequenceNumber> = None;

        for result in WalScanner::open(&self.path)?.scan() {
            let record = result?;
            last_seen_seq = Some(record.seq);

            // Tombstone records (empty payload) are kept to preserve seq continuity.
            if record.payload.is_empty() {
                kept.push((record.seq, record.payload));
                continue;
            }

            let event: Event = rkyv::from_bytes(&record.payload)
                .map_err(|e| ZamError::Serialization(format!("{}", e)))?;

            let below_frontier = frontier
                .get(&event.origin_node.0)
                .map(|&frontier_seq| event.seq <= frontier_seq)
                .unwrap_or(false);

            if below_frontier {
                dropped += 1;
            } else {
                kept.push((record.seq, record.payload));
            }
        }

        if dropped == 0 {
            return Ok(0);
        }

        // If every remaining record is a tombstone (or kept is empty), replace them
        // with a single tombstone at the last WAL seq so WalWriter resumes correctly.
        let all_tombstones = kept.iter().all(|(_, p)| p.is_empty());
        if all_tombstones {
            kept.clear();
            if let Some(last_seq) = last_seen_seq {
                kept.push((last_seq, Vec::new()));
            }
        }

        let tmp = self.path.with_extension("wal.tmp");
        {
            let mut w = WalWriter::open(&tmp, SequenceNumber::ZERO)?;
            for (seq, payload) in &kept {
                w.append_at_seq(*seq, payload)?;
            }
            w.sync()?;
        }

        // Atomic replace -- on Windows rename fails if the destination exists.
        if self.path.exists() {
            std::fs::remove_file(&self.path)?;
        }
        std::fs::rename(&tmp, &self.path)?;

        let (last_seq, end_pos) = WalScanner::recover(&self.path)?;
        let next_seq = last_seq.map(|s| s.next()).unwrap_or(SequenceNumber::ZERO);

        let actual_len = std::fs::metadata(&self.path).map(|m| m.len()).unwrap_or(0);
        if actual_len > end_pos {
            std::fs::OpenOptions::new()
                .write(true)
                .open(&self.path)?
                .set_len(end_pos)?;
        }

        self.writer = WalWriter::open(&self.path, next_seq)?;
        Ok(dropped)
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
        let iter = scanner.scan().filter_map(|res| -> Option<ZamResult<Event>> {
            let record = match res {
                Ok(r) => r,
                Err(e) => return Some(Err(e)),
            };
            if record.payload.is_empty() {
                return None; // tombstone written by compact() to preserve seq continuity
            }
            Some(
                rkyv::from_bytes::<Event>(&record.payload)
                    .map_err(|e| ZamError::Serialization(format!("{}", e))),
            )
        });
        Ok(Box::new(iter))
    }

    fn sync(&mut self) -> ZamResult<()> {
        self.writer.sync().map_err(Into::into)
    }
}
