use zamsync_core::{Event, SequenceNumber, ZamResult};
use zamsync_storage::{ZamEngine, StateStore};
use tempfile::tempdir;
use std::collections::HashMap;

// --- DOMAIN LAYER (Medical) ---
// This would normally live in a separate crate or application module.

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
        // Namespace 1 = Medical
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

// --- INFRASTRUCTURE TEST ---

#[test]
fn test_engine_domain_agnostic() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let wal_path = dir.path().join("zamsync.wal");

    let mut state = MedicalState::default();
    let mut engine = ZamEngine::open(&wal_path, state)?;

    // Submit a domain event wrapped in a generic event
    let med_event = MedicalEvent::UpsertPatient {
        id: "p1".into(),
        name: "Dorji".into(),
    };
    
    engine.submit(Event {
        event_type: 1,
        payload: serde_json::to_vec(&med_event)?,
    })?;

    assert_eq!(engine.state().patients["p1"], "Dorji");

    // Recovery test
    drop(engine);
    let engine_recovered = ZamEngine::open(&wal_path, MedicalState::default())?;
    assert_eq!(engine_recovered.state().patients["p1"], "Dorji");

    Ok(())
}
