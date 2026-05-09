use zamsync_core::{Event, SequenceNumber};
use zamsync_storage::{ZamEngine, MemoryStateStore};
use tempfile::tempdir;

#[test]
fn test_engine_end_to_end() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let wal_path = dir.path().join("zamsync.wal");

    // 1. Start Engine
    let mut engine = ZamEngine::open(&wal_path, MemoryStateStore::default())?;

    // 2. Submit Events
    engine.submit(Event::UpsertPatient {
        id: "p1".into(),
        name: "Dorji Namgay".into(),
        age: 42,
        location: "Thimphu".into(),
    })?;

    engine.submit(Event::UpdateInventory {
        medication_id: "paracetamol".into(),
        delta: 100,
    })?;

    // 3. Verify State
    {
        let state = engine.state();
        assert_eq!(state.patients["p1"].name, "Dorji Namgay");
        assert_eq!(state.inventory["paracetamol"], 100);
    }

    // 4. Simulate Shutdown and Recovery
    drop(engine);
    
    let engine_recovered = ZamEngine::open(&wal_path, MemoryStateStore::default())?;
    
    // 5. Verify Recovered State
    {
        let state = engine_recovered.state();
        assert_eq!(state.patients["p1"].name, "Dorji Namgay");
        assert_eq!(state.inventory["paracetamol"], 100);
    }

    Ok(())
}
