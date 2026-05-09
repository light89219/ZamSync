use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use zamsync_core::{ReplicationState, PeerSyncState, NodeId, SequenceNumber, ZamResult, ZamError};

pub struct PeerManager {
    path: std::path::PathBuf,
    state: ReplicationState,
}

impl PeerManager {
    pub fn open(path: impl AsRef<Path>, self_id: NodeId) -> ZamResult<Self> {
        let path = path.as_ref().to_path_buf();
        let state = if path.exists() {
            let mut file = File::open(&path)?;
            let mut buffer = Vec::new();
            file.read_to_end(&mut buffer)?;
            rkyv::from_bytes(&buffer)
                .map_err(|e| ZamError::Storage(format!("Failed to load peer state: {}", e)))?
        } else {
            ReplicationState {
                self_id,
                peers: std::collections::HashMap::new(),
            }
        };

        Ok(Self { path, state })
    }

    pub fn update_received(&mut self, peer_id: NodeId, seq: SequenceNumber) -> ZamResult<()> {
        let peer = self.state.peers.entry(peer_id.0).or_default();
        peer.last_received = Some(seq);
        self.save()
    }

    pub fn update_acked(&mut self, peer_id: NodeId, seq: SequenceNumber) -> ZamResult<()> {
        let peer = self.state.peers.entry(peer_id.0).or_default();
        peer.last_acked = Some(seq);
        self.save()
    }

    pub fn get_peer_state(&self, peer_id: NodeId) -> Option<&PeerSyncState> {
        self.state.peers.get(&peer_id.0)
    }

    pub fn save(&self) -> ZamResult<()> {
        let bytes = rkyv::to_bytes::<_, 256>(&self.state)
            .map_err(|e| ZamError::Serialization(e.to_string()))?;
        
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&self.path)?;
        
        file.write_all(&bytes)?;
        file.sync_all()?;
        Ok(())
    }
}
