use zamsync_core::{Event, SequenceNumber, NodeId, ZamResult, Hlc};
use zamsync_storage::{ZamEngine, StateStore};
use tempfile::tempdir;
use std::collections::HashMap;

// --- DOMAIN LAYER (Medical) ---
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
enum MedicalEvent {
    UpsertPatient { id: String, name: String },
}

#[derive(Default)]
struct MedicalState {
    patients: HashMap<String, String>,
    last_seq: Option<SequenceNumber>,
}

impl StateStore for MedicalState {
    fn apply_event(&mut self, seq: SequenceNumber, event: &Event) -> ZamResult<()> {
        if event.event_type == 1 {
            let med_event: MedicalEvent = serde_json::from_slice(&event.payload)
                .map_err(|e| zamsync_core::ZamError::Serialization(e.to_string()))?;
            
            match med_event {
                MedicalEvent::UpsertPatient { id, name } => {
                    self.patients.insert(id, name);
                }
            }
        }
        self.last_seq = Some(seq);
        Ok(())
    }

    fn last_applied_seq(&self) -> Option<SequenceNumber> {
        self.last_seq
    }
}

#[test]
fn test_engine_distributed_identity() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let wal_path = dir.path().join("zamsync.wal");
    let self_id = NodeId(1);

    let mut engine = ZamEngine::open(&wal_path, self_id, MedicalState::default())?;

    // 1. Submit local event
    let med_event = MedicalEvent::UpsertPatient {
        id: "p1".into(),
        name: "Dorji".into(),
    };
    
    let seq = engine.submit(1, serde_json::to_vec(&med_event)?)?;
    assert_eq!(seq.0, 0);
    assert_eq!(engine.state().patients["p1"], "Dorji");

    // 2. Simulate shutdown and recovery
    drop(engine);
    let engine_recovered = ZamEngine::open(&wal_path, self_id, MedicalState::default())?;
    assert_eq!(engine_recovered.state().patients["p1"], "Dorji");

    Ok(())
}

#[test]
fn test_replicated_event_handling() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let wal_path = dir.path().join("zamsync.wal");
    let self_id = NodeId(1);
    let peer_id = NodeId(2);

    let mut engine = ZamEngine::open(&wal_path, self_id, MedicalState::default())?;

    // Create a "replicated" event from peer_id
    let event = Event {
        origin_node: peer_id,
        seq: SequenceNumber(100),
        hlc: Hlc::new(12345, 0),
        event_type: 1,
        payload: serde_json::to_vec(&MedicalEvent::UpsertPatient {
            id: "p2".into(),
            name: "Sangay".into(),
        })?,
    };

    engine.apply_replicated(event)?;
    assert_eq!(engine.state().patients["p2"], "Sangay");

    Ok(())
}
