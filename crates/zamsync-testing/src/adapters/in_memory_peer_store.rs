use std::collections::HashMap;
use zamsync_core::ports::PeerStore;
use zamsync_core::{NodeId, ReplicationState, VersionVector, ZamResult};

pub struct InMemoryPeerStore {
    state: ReplicationState,
}

impl InMemoryPeerStore {
    pub fn new(self_id: NodeId) -> Self {
        Self {
            state: ReplicationState {
                self_id,
                local_vv: VersionVector::new(),
                peers: HashMap::new(),
            },
        }
    }
}

impl PeerStore for InMemoryPeerStore {
    fn load(&self) -> ZamResult<ReplicationState> {
        Ok(self.state.clone())
    }

    fn save(&mut self, state: &ReplicationState) -> ZamResult<()> {
        self.state = state.clone();
        Ok(())
    }
}
