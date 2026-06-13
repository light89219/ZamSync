use std::env;
use std::path::PathBuf;
use zamsync_core::ports::StateStore;
use zamsync_core::{Event, NodeId, SequenceNumber, ZamResult};
use zamsync_storage::ZamEngine;

#[derive(Default)]
struct EventCounter {
    count: usize,
    last_seq: Option<SequenceNumber>,
}

impl StateStore for EventCounter {
    fn apply_event(&mut self, seq: SequenceNumber, _event: &Event) -> ZamResult<()> {
        self.count += 1;
        self.last_seq = Some(seq);
        Ok(())
    }
    fn last_applied_seq(&self) -> Option<SequenceNumber> {
        self.last_seq
    }
}

fn usage() {
    eprintln!(
        "Usage:
  zamsync info <data-dir>          Show node status
  zamsync submit <data-dir> <msg>  Append an event with a text payload"
    );
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    match args.get(1).map(String::as_str) {
        Some("info") => {
            let dir = data_dir(&args, 2)?;
            let node_id = node_id_from_dir(&dir);
            let engine = ZamEngine::open_wal(&dir, node_id, EventCounter::default())?;

            println!("node_id  : {}", node_id.0);
            println!("data_dir : {}", dir.display());
            println!("events   : {}", engine.state().count);
            let vv = &engine.replication_state().local_vv;
            if vv.entries.is_empty() {
                println!("vv       : (empty)");
            } else {
                for (node, seq) in &vv.entries {
                    println!("vv       : node {} @ seq {}", node, seq.0);
                }
            }
        }
        Some("submit") => {
            let dir = data_dir(&args, 2)?;
            let payload = args
                .get(3)
                .ok_or("missing payload argument")?
                .as_bytes()
                .to_vec();
            let node_id = node_id_from_dir(&dir);
            let mut engine = ZamEngine::open_wal(&dir, node_id, EventCounter::default())?;
            let seq = engine.submit(1, payload)?;
            engine.sync()?;
            println!("submitted seq={}", seq.0);
        }
        _ => {
            usage();
            std::process::exit(1);
        }
    }

    Ok(())
}

fn data_dir(args: &[String], pos: usize) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let path = PathBuf::from(args.get(pos).ok_or("missing data-dir argument")?);
    std::fs::create_dir_all(&path)?;
    Ok(path)
}

fn node_id_from_dir(dir: &std::path::Path) -> NodeId {
    let id_file = dir.join(".node_id");
    if let Ok(bytes) = std::fs::read(&id_file) {
        if let Ok(s) = std::str::from_utf8(&bytes) {
            if let Ok(n) = s.trim().parse::<u32>() {
                return NodeId(n);
            }
        }
    }
    let id: u32 = rand_u32();
    let _ = std::fs::write(&id_file, id.to_string());
    NodeId(id)
}

fn rand_u32() -> u32 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::time::SystemTime;
    let mut h = DefaultHasher::new();
    SystemTime::now().hash(&mut h);
    std::process::id().hash(&mut h);
    h.finish() as u32
}
