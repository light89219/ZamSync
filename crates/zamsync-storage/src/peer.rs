use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use zamsync_core::{ReplicationState, VersionVector, NodeId, SequenceNumber, ZamResult, ZamError};

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
                local_vv: VersionVector::new(),
                peers: std::collections::HashMap::new(),
            }
        };

        Ok(Self { path, state })
    }

    /// Update our local knowledge about a specific node.
    pub fn update_local_knowledge(&mut self, origin_id: NodeId, seq: SequenceNumber) -> ZamResult<()> {
        self.state.local_vv.update(origin_id, seq);
        self.save()
    }

    /// Update what we know about a peer's knowledge frontier.
    pub fn update_peer_knowledge(&mut self, peer_id: NodeId, vv: VersionVector) -> ZamResult<()> {
        let peer = self.state.peers.entry(peer_id.0).or_default();
        peer.known_vv = vv;
        self.save()
    }

    pub fn update_acked(&mut self, peer_id: NodeId, seq: SequenceNumber) -> ZamResult<()> {
        let peer = self.state.peers.entry(peer_id.0).or_default();
        peer.last_acked = Some(seq);
        self.save()
    }

    pub fn local_vv(&self) -> &VersionVector {
        &self.state.local_vv
    }

    pub fn get_peer_vv(&self, peer_id: NodeId) -> Option<&VersionVector> {
        self.state.peers.get(&peer_id.0).map(|p| &p.known_vv)
    }

    pub fn save(&self) -> ZamResult<()> {
        let bytes = rkyv::to_bytes::<_, 1024>(&self.state)
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
