use std::sync::{Arc, Mutex};
use tempfile::tempdir;
use zamsync_core::ports::StateStore;
use zamsync_core::{Event, NodeId, SequenceNumber, ZamResult};
use zamsync_storage::{FilePeerStore, WalEventStore, ZamEngine};
use zamsync_testing::run_direct_sync;

type Engine = ZamEngine<WalEventStore, FilePeerStore, Counter>;

#[derive(Default)]
struct Counter {
    count: usize,
}

impl StateStore for Counter {
    fn apply_event(&mut self, _seq: SequenceNumber, _event: &Event) -> ZamResult<()> {
        self.count += 1;
        Ok(())
    }
    fn last_applied_seq(&self) -> Option<SequenceNumber> {
        None
    }
}

fn open_engine(dir: &tempfile::TempDir, node: NodeId) -> ZamResult<Engine> {
    ZamEngine::open_wal(dir.path(), node, Counter::default())
}

/// Multiple threads sharing an engine via Mutex submit concurrently.
/// No events must be lost and all sequence numbers must be unique.
#[test]
fn test_concurrent_writes_no_lost_events() {
    const THREADS: usize = 8;
    const EVENTS_PER_THREAD: usize = 100;
    const TOTAL: usize = THREADS * EVENTS_PER_THREAD;

    let dir = tempdir().unwrap();
    let node = NodeId(1);
    let engine = Arc::new(Mutex::new(open_engine(&dir, node).unwrap()));

    let handles: Vec<_> = (0..THREADS)
        .map(|t| {
            let engine = Arc::clone(&engine);
            std::thread::spawn(move || {
                for i in 0..EVENTS_PER_THREAD {
                    let payload = format!("t{t}-e{i}").into_bytes();
                    engine.lock().unwrap().submit(1, payload).unwrap();
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    engine.lock().unwrap().sync().unwrap();

    // Reopen: open_wal replays all WAL records into state
    let final_engine = open_engine(&dir, node).unwrap();
    assert_eq!(
        final_engine.state().count,
        TOTAL,
        "all {TOTAL} events must survive: no losses under concurrent submit"
    );

    // All sequence numbers must be unique (no double-write)
    let scan_engine = open_engine(&dir, node).unwrap();
    let mut seqs: Vec<u64> = scan_engine
        .scan_events()
        .unwrap()
        .filter_map(|r| r.ok())
        .map(|e| e.seq.0)
        .collect();
    let original_len = seqs.len();
    seqs.sort_unstable();
    seqs.dedup();
    assert_eq!(
        seqs.len(),
        original_len,
        "duplicate sequence numbers found -- concurrent WAL writes corrupted"
    );
    assert_eq!(seqs.len(), TOTAL, "sequence count must equal submitted count");
}

/// A node compacts while a new peer connects immediately after.
/// This simulates the race between compaction and an incoming sync.
/// The new peer must receive at least the post-compaction events with no
/// corruption on either side.
#[test]
fn test_compaction_during_active_sync() {
    let dir_a = tempdir().unwrap();
    let dir_b = tempdir().unwrap();
    let dir_c = tempdir().unwrap();
    let node_a = NodeId(20);
    let node_b = NodeId(21);
    let node_c = NodeId(22);

    // A submits 10 events
    {
        let mut engine_a = open_engine(&dir_a, node_a).unwrap();
        for i in 0..10u32 {
            engine_a
                .submit(1, format!("event-{i}").into_bytes())
                .unwrap();
        }
        engine_a.sync().unwrap();
    }

    // B syncs with A -- this lets A know B confirmed all 10 events
    {
        let mut engine_a = open_engine(&dir_a, node_a).unwrap();
        let mut engine_b = open_engine(&dir_b, node_b).unwrap();
        run_direct_sync(&mut engine_a, &mut engine_b).unwrap();
        engine_a.sync().unwrap();
        engine_b.sync().unwrap();
    }

    // A compacts (B confirmed all events) and immediately submits a new event --
    // this is the moment C would be connecting in the real race
    let dropped = {
        let mut engine_a = open_engine(&dir_a, node_a).unwrap();
        let d = engine_a.compact().unwrap();
        engine_a.submit(1, b"post-compact".to_vec()).unwrap();
        engine_a.sync().unwrap();
        d
    };
    assert_eq!(dropped, 10, "all 10 pre-compaction events must be dropped");

    // C connects right after compaction completes -- the WAL must still be
    // consistent and C must receive the post-compact event
    {
        let mut engine_a = open_engine(&dir_a, node_a).unwrap();
        let mut engine_c = open_engine(&dir_c, node_c).unwrap();
        run_direct_sync(&mut engine_a, &mut engine_c).unwrap();
        engine_c.sync().unwrap();
    }

    let engine_c = open_engine(&dir_c, node_c).unwrap();
    assert!(
        engine_c.state().count >= 1,
        "C must receive at least the post-compact event; got {}",
        engine_c.state().count
    );

    // A's WAL must be intact -- scan must not error
    let engine_a = open_engine(&dir_a, node_a).unwrap();
    let seqs: Vec<u64> = engine_a
        .scan_events()
        .unwrap()
        .map(|r| r.expect("WAL corruption on A after compaction").seq.0)
        .collect();
    assert!(
        !seqs.is_empty(),
        "A's WAL must not be empty or corrupt after compaction"
    );
}
