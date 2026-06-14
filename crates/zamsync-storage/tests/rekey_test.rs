use std::sync::Arc;
use tempfile::tempdir;
use zamsync_core::{SequenceNumber, ZamResult};
use zamsync_storage::encryption::EncryptionKey;
use zamsync_storage::wal::{WalScanner, WalWriter};

#[test]
fn test_wal_rekey_all_records_readable_with_new_key() -> ZamResult<()> {
    let dir = tempdir()?;
    let wal_path = dir.path().join("events.wal");
    let tmp_path = wal_path.with_extension("wal.rekey");

    let old_key = Arc::new(EncryptionKey::generate()?);
    let new_key = Arc::new(EncryptionKey::generate()?);

    let payloads: &[&[u8]] = &[
        b"patient-A",
        b"patient-B",
        b"patient-C",
        b"patient-D",
        b"patient-E",
    ];

    // Write 5 records encrypted with old_key.
    {
        let mut w =
            WalWriter::open_encrypted(&wal_path, SequenceNumber::ZERO, Arc::clone(&old_key))?;
        for p in payloads {
            w.append(p)?;
        }
        w.sync()?;
    }

    // Rekey: decrypt with old_key, re-encrypt with new_key (same logic as CLI rekey command).
    {
        let scanner = WalScanner::open_encrypted(&wal_path, Arc::clone(&old_key))?;
        let records: Vec<_> = scanner.scan().collect::<ZamResult<_>>()?;
        assert_eq!(records.len(), 5, "old key must read all records");

        let start_seq = records.first().map(|r| r.seq).unwrap_or_default();
        let mut writer =
            WalWriter::open_encrypted(&tmp_path, start_seq, Arc::clone(&new_key))?;
        for r in &records {
            writer.append_at_seq(r.seq, &r.payload)?;
        }
        writer.sync()?;
    }
    std::fs::remove_file(&wal_path)?;
    std::fs::rename(&tmp_path, &wal_path)?;

    // All 5 records must be readable with new_key.
    {
        let scanner = WalScanner::open_encrypted(&wal_path, Arc::clone(&new_key))?;
        let records: Vec<_> = scanner.scan().collect::<ZamResult<_>>()?;
        assert_eq!(records.len(), 5, "new key must read all re-keyed records");
        for (record, expected) in records.iter().zip(payloads.iter()) {
            assert_eq!(&record.payload, expected, "payload must survive rekey intact");
        }
        assert_eq!(records[0].seq, SequenceNumber(0));
        assert_eq!(records[4].seq, SequenceNumber(4));
    }

    Ok(())
}

#[test]
fn test_wal_rekey_old_key_cannot_read_rekeyed_wal() -> ZamResult<()> {
    let dir = tempdir()?;
    let wal_path = dir.path().join("events.wal");
    let tmp_path = wal_path.with_extension("wal.rekey");

    let old_key = Arc::new(EncryptionKey::generate()?);
    let new_key = Arc::new(EncryptionKey::generate()?);

    {
        let mut w =
            WalWriter::open_encrypted(&wal_path, SequenceNumber::ZERO, Arc::clone(&old_key))?;
        w.append(b"secret-record")?;
        w.sync()?;
    }

    // Rekey to new_key.
    {
        let scanner = WalScanner::open_encrypted(&wal_path, Arc::clone(&old_key))?;
        let records: Vec<_> = scanner.scan().collect::<ZamResult<_>>()?;
        let start = records.first().map(|r| r.seq).unwrap_or_default();
        let mut writer = WalWriter::open_encrypted(&tmp_path, start, Arc::clone(&new_key))?;
        for r in &records {
            writer.append_at_seq(r.seq, &r.payload)?;
        }
        writer.sync()?;
    }
    std::fs::remove_file(&wal_path)?;
    std::fs::rename(&tmp_path, &wal_path)?;

    // Old key must fail: AEAD authentication will reject the ciphertext.
    let scanner = WalScanner::open_encrypted(&wal_path, Arc::clone(&old_key))?;
    let result: ZamResult<Vec<_>> = scanner.scan().collect();
    assert!(result.is_err(), "old key must not decrypt a re-keyed WAL");

    Ok(())
}

#[test]
fn test_wal_rekey_preserves_seq_numbers() -> ZamResult<()> {
    let dir = tempdir()?;
    let wal_path = dir.path().join("events.wal");
    let tmp_path = wal_path.with_extension("wal.rekey");

    let old_key = Arc::new(EncryptionKey::generate()?);
    let new_key = Arc::new(EncryptionKey::generate()?);

    // Write records with non-contiguous seq numbers (e.g. after a compaction).
    {
        let mut w =
            WalWriter::open_encrypted(&wal_path, SequenceNumber(10), Arc::clone(&old_key))?;
        w.append_at_seq(SequenceNumber(10), b"r10")?;
        w.append_at_seq(SequenceNumber(20), b"r20")?;
        w.append_at_seq(SequenceNumber(30), b"r30")?;
        w.sync()?;
    }

    {
        let scanner = WalScanner::open_encrypted(&wal_path, Arc::clone(&old_key))?;
        let records: Vec<_> = scanner.scan().collect::<ZamResult<_>>()?;
        let start = records.first().map(|r| r.seq).unwrap_or_default();
        let mut writer = WalWriter::open_encrypted(&tmp_path, start, Arc::clone(&new_key))?;
        for r in &records {
            writer.append_at_seq(r.seq, &r.payload)?;
        }
        writer.sync()?;
    }
    std::fs::remove_file(&wal_path)?;
    std::fs::rename(&tmp_path, &wal_path)?;

    let scanner = WalScanner::open_encrypted(&wal_path, Arc::clone(&new_key))?;
    let records: Vec<_> = scanner.scan().collect::<ZamResult<_>>()?;
    assert_eq!(records.len(), 3);
    assert_eq!(records[0].seq, SequenceNumber(10));
    assert_eq!(records[1].seq, SequenceNumber(20));
    assert_eq!(records[2].seq, SequenceNumber(30));

    Ok(())
}
