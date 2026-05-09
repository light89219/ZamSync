use zamsync_core::{NodeId, SequenceNumber, VersionVector};
use zamsync_storage::PeerManager;
use tempfile::tempdir;

#[test]
fn test_version_vector_persistence() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let state_path = dir.path().join("peers.db");
    let self_id = NodeId(1);
    let peer_id = NodeId(2);
    let node_c = NodeId(3);

    // 1. Initialize and update VV
    {
        let mut manager = PeerManager::open(&state_path, self_id)?;
        manager.update_local_knowledge(self_id, SequenceNumber(10))?;
        manager.update_local_knowledge(node_c, SequenceNumber(5))?;
        
        let mut peer_vv = VersionVector::new();
        peer_vv.update(self_id, SequenceNumber(8));
        manager.update_peer_knowledge(peer_id, peer_vv)?;
    }

    // 2. Reload and verify
    {
        let manager = PeerManager::open(&state_path, self_id)?;
        assert_eq!(manager.local_vv().get(self_id), SequenceNumber(10));
        assert_eq!(manager.local_vv().get(node_c), SequenceNumber(5));
        
        let peer_vv = manager.get_peer_vv(peer_id).unwrap();
        assert_eq!(peer_vv.get(self_id), SequenceNumber(8));
        assert_eq!(peer_vv.get(NodeId(99)), SequenceNumber(0)); // Unknown node
    }

    Ok(())
}

#[test]
fn test_vv_gap_discovery() {
    let mut local = VersionVector::new();
    local.update(NodeId(1), SequenceNumber(10));
    local.update(NodeId(2), SequenceNumber(5));

    let mut remote = VersionVector::new();
    remote.update(NodeId(1), SequenceNumber(12)); // Ahead on 1
    remote.update(NodeId(2), SequenceNumber(5));  // Equal on 2
    remote.update(NodeId(3), SequenceNumber(3));  // Ahead on 3 (new)

    let gaps = local.find_gaps(&remote);
    assert_eq!(gaps.len(), 2);
    
    // Check for Node 1
    assert!(gaps.iter().any(|(id, seq)| id.0 == 1 && seq.0 == 10));
    // Check for Node 3
    assert!(gaps.iter().any(|(id, seq)| id.0 == 3 && seq.0 == 0));
}
