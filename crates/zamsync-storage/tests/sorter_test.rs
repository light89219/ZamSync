use zamsync_core::{Event, Hlc, NodeId, SequenceNumber, ZamResult};
use zamsync_storage::LogSorter;

#[test]
fn test_log_sorter_deterministic_merge() -> Result<(), Box<dyn std::error::Error>> {
    let node_a = NodeId(1);
    let node_b = NodeId(2);

    // Stream 1 (Node A)
    let s1 = vec![
        Ok(Event {
            origin_node: node_a,
            seq: SequenceNumber(1),
            hlc: Hlc::new(100, 0),
            event_type: 1,
            payload: b"a1".to_vec(),
        }),
        Ok(Event {
            origin_node: node_a,
            seq: SequenceNumber(2),
            hlc: Hlc::new(105, 0),
            event_type: 1,
            payload: b"a2".to_vec(),
        }),
    ];

    // Stream 2 (Node B)
    let s2 = vec![
        Ok(Event {
            origin_node: node_b,
            seq: SequenceNumber(1),
            hlc: Hlc::new(102, 0), // Concurrent with a1/a2, but physically after a1
            event_type: 1,
            payload: b"b1".to_vec(),
        }),
        Ok(Event {
            origin_node: node_b,
            seq: SequenceNumber(2),
            hlc: Hlc::new(105, 1), // Later than a2 (same physical, higher logical)
            event_type: 1,
            payload: b"b2".to_vec(),
        }),
    ];

    let sorter = LogSorter::new(vec![s1.into_iter(), s2.into_iter()])?;
    let results: Vec<Event> = sorter.collect::<ZamResult<Vec<_>>>()?;

    // Expected Order:
    // 1. a1 (HLC 100,0)
    // 2. b1 (HLC 102,0)
    // 3. a2 (HLC 105,0)
    // 4. b2 (HLC 105,1)
    
    assert_eq!(results[0].payload, b"a1");
    assert_eq!(results[1].payload, b"b1");
    assert_eq!(results[2].payload, b"a2");
    assert_eq!(results[3].payload, b"b2");

    Ok(())
}

#[test]
fn test_log_sorter_tie_break_with_node_id() -> Result<(), Box<dyn std::error::Error>> {
    let node_a = NodeId(1);
    let node_b = NodeId(2);

    // Two concurrent events with EXACT SAME HLC
    let s1 = vec![Ok(Event {
        origin_node: node_a,
        seq: SequenceNumber(1),
        hlc: Hlc::new(100, 0),
        event_type: 1,
        payload: b"a".to_vec(),
    })];

    let s2 = vec![Ok(Event {
        origin_node: node_b,
        seq: SequenceNumber(1),
        hlc: Hlc::new(100, 0),
        event_type: 1,
        payload: b"b".to_vec(),
    })];

    let sorter = LogSorter::new(vec![s1.into_iter(), s2.into_iter()])?;
    let results: Vec<Event> = sorter.collect::<ZamResult<Vec<_>>>()?;

    // Deterministic tie-break: Node 1 before Node 2 (assuming lower NodeId comes first in our Ord impl)
    // My Ord impl for IndexedEvent: 
    // match self.event.hlc.cmp(&other.event.hlc) {
    //     Ordering::Equal => other.event.origin_node.0.cmp(&self.event.origin_node.0), // Reverted for Min-Heap
    //     other_order => other_order.reverse(),
    // }
    
    // other.origin_node.0.cmp(&self.origin_node.0)
    // If self is 1 and other is 2: 2.cmp(1) = Greater. So self (1) is "smaller" in the heap logic.
    // Correct. Node 1 should come before Node 2.

    assert_eq!(results[0].origin_node.0, 1);
    assert_eq!(results[1].origin_node.0, 2);

    Ok(())
}
