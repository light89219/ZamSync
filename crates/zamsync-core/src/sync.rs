use std::collections::HashMap;
use rkyv::{Archive, Deserialize, Serialize};
use crate::{NodeId, SequenceNumber};

/// A Version Vector tracks the "frontier" of knowledge for all nodes in the network.
#[derive(Archive, Deserialize, Serialize, Debug, Clone, Default, PartialEq, Eq)]
#[archive(check_bytes)]
pub struct VersionVector {
    /// Mapping from NodeId to the highest SequenceNumber seen from that node.
    pub entries: HashMap<u32, SequenceNumber>,
}

impl VersionVector {
    pub fn new() -> Self {
        Self::default()
    }

    /// Update the vector with a new sequence number from a node.
    pub fn update(&mut self, node_id: NodeId, seq: SequenceNumber) {
        let entry = self.entries.entry(node_id.0).or_insert(SequenceNumber::ZERO);
        if seq > *entry {
            *entry = seq;
        }
    }

    /// Get the sequence number for a specific node.
    pub fn get(&self, node_id: NodeId) -> SequenceNumber {
        self.entries.get(&node_id.0).cloned().unwrap_or(SequenceNumber::ZERO)
    }

    /// Compare with another VersionVector to find what this vector is missing.
    /// Returns a list of (NodeId, last_known_seq_here) for nodes where 'other' is ahead.
    pub fn find_gaps(&self, other: &VersionVector) -> Vec<(NodeId, SequenceNumber)> {
        let mut gaps = Vec::new();
        for (node_id_raw, other_seq) in &other.entries {
            let node_id = NodeId(*node_id_raw);
            let local_seq = self.get(node_id);
            if *other_seq > local_seq {
                gaps.push((node_id, local_seq));
            }
        }
        gaps
    }
}

/// State of synchronization with a specific peer.
#[derive(Archive, Deserialize, Serialize, Debug, Clone, Default)]
#[archive(check_bytes)]
pub struct PeerSyncState {
    /// The last full Version Vector we received from this peer.
    pub known_vv: VersionVector,
    /// The last sequence number of OURS that this peer has acknowledged.
    pub last_acked: Option<SequenceNumber>,
}

/// A collection of sync states for all known peers.
#[derive(Archive, Deserialize, Serialize, Debug, Clone, Default)]
#[archive(check_bytes)]
pub struct ReplicationState {
    pub self_id: NodeId,
    /// Our current global knowledge.
    pub local_vv: VersionVector,
    /// Knowledge of our peers.
    pub peers: HashMap<u32, PeerSyncState>,
}

/// Messages exchanged between nodes to coordinate replication.
#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive(check_bytes)]
pub enum SyncMessage {
    /// Initial message to exchange Version Vectors.
    Handshake {
        node_id: NodeId,
        vv: VersionVector,
    },
    /// Request for specific events from a node.
    PullRequest {
        origin_node: NodeId,
        start_seq: SequenceNumber,
        limit: u32,
    },
    /// Batch of events being pushed or returned from a Pull.
    EventBatch {
        origin_node: NodeId,
        events: Vec<(SequenceNumber, crate::Event)>,
    },
}
