use std::fs::OpenOptions;
use std::io::{Seek, SeekFrom, Write};
use tempfile::tempdir;
use zamsync_core::ports::StateStore;
use zamsync_core::{Event, NodeId, SequenceNumber, ZamResult};
use zamsync_storage::wal::{WalScanner, WalWriter, WAL_HEADER_SIZE};
use zamsync_storage::{FilePeerStore, WalEventStore, ZamEngine};

#[derive(Default)]
struct Counter(usize);

impl StateStore for Counter {
    fn apply_event(&mut self, _seq: SequenceNumber, _event: &Event) -> ZamResult<()> {
        self.0 += 1;
        Ok(())
    }
    fn last_applied_seq(&self) -> Option<SequenceNumber> {
        None
    }
}

fn open_engine(
    dir: &tempfile::TempDir,
    node: NodeId,
) -> ZamResult<ZamEngine<WalEventStore, FilePeerStore, Counter>> {
    ZamEngine::open_wal(dir.path(), node, Counter::default())
}

// ---------------------------------------------------------------------------
// WAL-level crash scenarios (use WalWriter directly -- known payload sizes)
// ---------------------------------------------------------------------------

/// Truncating a single byte from the payload stops recovery before that record.
#[test]
fn test_wal_truncate_mid_payload() -> ZamResult<()> {
    let dir = tempdir()?;
    let path = dir.path().join("test.wal");

    let mut w = WalWriter::open(&path, SequenceNumber::ZERO)?;
    w.append(b"first")?;
    w.append(b"second")?;
    w.sync()?;

    // Cut one byte off the end (inside the second record's payload)
    let len = std::fs::metadata(&path)?.len();
    OpenOptions::new()
        .write(true)
        .open(&path)?
        .set_len(len - 1)?;

    let (last_seq, _) = WalScanner::recover(&path)?;
    assert_eq!(
        last_seq,
        Some(SequenceNumber(0)),
        "only first record survives"
    );

    Ok(())
}

/// Truncating to exactly after the first record leaves it intact.
#[test]
fn test_wal_truncate_after_first_record() -> ZamResult<()> {
    let dir = tempdir()?;
    let path = dir.path().join("test.wal");

    let payload = b"keep";
    let mut w = WalWriter::open(&path, SequenceNumber::ZERO)?;
    w.append(payload)?;
    w.append(b"drop")?;
    w.sync()?;

    let first_size = (WAL_HEADER_SIZE + payload.len()) as u64;
    OpenOptions::new()
        .write(true)
        .open(&path)?
        .set_len(first_size)?;

    let (last_seq, pos) = WalScanner::recover(&path)?;
    assert_eq!(last_seq, Some(SequenceNumber(0)));
    assert_eq!(pos, first_size);

    Ok(())
}

/// Corrupting the CRC of the second record makes recovery stop after the first.
#[test]
fn test_wal_corrupt_crc_stops_recovery() -> ZamResult<()> {
    let dir = tempdir()?;
    let path = dir.path().join("test.wal");

    let first_payload = b"ok";
    let mut w = WalWriter::open(&path, SequenceNumber::ZERO)?;
    w.append(first_payload)?;
    w.append(b"bad")?;
    w.sync()?;

    // Flip a byte inside the second record's payload
    let second_record_start = WAL_HEADER_SIZE + first_payload.len();
    let corrupt_offset = (second_record_start + WAL_HEADER_SIZE + 1) as u64;
    {
        let mut f = OpenOptions::new().write(true).open(&path)?;
        f.seek(SeekFrom::Start(corrupt_offset))?;
        f.write_all(&[0xff])?;
    }

    let (last_seq, _) = WalScanner::recover(&path)?;
    assert_eq!(last_seq, Some(SequenceNumber(0)));

    Ok(())
}

// ---------------------------------------------------------------------------
// Engine-level crash scenarios
// ---------------------------------------------------------------------------

/// Snapshot WAL size after first event, write a second, truncate back.
/// Engine must recover only the first event.
#[test]
fn test_engine_recovers_after_wal_truncation() -> ZamResult<()> {
    let dir = tempdir()?;
    let wal_path = dir.path().join("events.wal");
    let node = NodeId(1);

    // Write first event and snapshot WAL size
    let size_after_first = {
        let mut e = open_engine(&dir, node)?;
        e.submit(1, b"A".to_vec())?;
        e.sync()?;
        std::fs::metadata(&wal_path)?.len()
    };

    // Write second and third events
    {
        let mut e = open_engine(&dir, node)?;
        e.submit(1, b"B".to_vec())?;
        e.submit(1, b"C".to_vec())?;
        e.sync()?;
    }

    // Truncate to just after the first event (simulates crash mid-write of B)
    OpenOptions::new()
        .write(true)
        .open(&wal_path)?
        .set_len(size_after_first)?;

    let mut recovered = open_engine(&dir, node)?;
    assert_eq!(recovered.state().0, 1, "only 1 event should survive");

    // Next submit must continue at seq 1, not seq 2 or 3
    let seq = recovered.submit(1, b"D".to_vec())?;
    assert_eq!(seq, SequenceNumber(1), "next seq must be 1 after recovery");

    Ok(())
}

/// VV rebuilt from WAL must reflect only the events that survived truncation.
#[test]
fn test_engine_vv_consistent_after_recovery() -> ZamResult<()> {
    let dir = tempdir()?;
    let wal_path = dir.path().join("events.wal");
    let node = NodeId(1);

    // Write first event and snapshot WAL size
    let size_after_first = {
        let mut e = open_engine(&dir, node)?;
        e.submit(1, b"x".to_vec())?;
        e.sync()?;
        std::fs::metadata(&wal_path)?.len()
    };

    // Write second event
    {
        let mut e = open_engine(&dir, node)?;
        e.submit(1, b"y".to_vec())?;
        e.sync()?;
    }

    // Truncate back to just after first event
    OpenOptions::new()
        .write(true)
        .open(&wal_path)?
        .set_len(size_after_first)?;

    let recovered = open_engine(&dir, node)?;
    let vv = &recovered.replication_state().local_vv;
    assert_eq!(
        vv.entries.get(&node.0),
        Some(&SequenceNumber(0)),
        "VV must reflect only seq 0 (x), not seq 1 (y)"
    );

    Ok(())
}

/// After recovery, submitting new events continues correctly and a fresh
/// engine can replay them all without errors.
#[test]
fn test_engine_append_after_recovery() -> ZamResult<()> {
    let dir = tempdir()?;
    let wal_path = dir.path().join("events.wal");
    let node = NodeId(1);

    let size_after_first = {
        let mut e = open_engine(&dir, node)?;
        e.submit(1, b"durable".to_vec())?;
        e.sync()?;
        std::fs::metadata(&wal_path)?.len()
    };

    // Simulate crash after partial second write
    {
        let mut e = open_engine(&dir, node)?;
        e.submit(1, b"lost".to_vec())?;
        // no sync -- also truncate to be sure
    }
    OpenOptions::new()
        .write(true)
        .open(&wal_path)?
        .set_len(size_after_first)?;

    // Recover and append a new event
    {
        let mut e = open_engine(&dir, node)?;
        e.submit(1, b"new".to_vec())?;
        e.sync()?;
    }

    // Re-open and verify exactly 2 events: "durable" and "new"
    let final_engine = open_engine(&dir, node)?;
    assert_eq!(final_engine.state().0, 2, "should have exactly 2 events");

    Ok(())
}
