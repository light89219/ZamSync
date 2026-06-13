use zamsync_core::ports::{EventStore, PeerStore, StateStore};
use zamsync_core::ZamResult;
use zamsync_storage::ZamEngine;

/// Synchronises two engines in both directions without a transport layer.
/// Returns `(events_applied_to_a, events_applied_to_b)`.
///
/// Useful for convergence tests that do not need to exercise the wire protocol.
pub fn run_direct_sync<E1, P1, S1, E2, P2, S2>(
    engine_a: &mut ZamEngine<E1, P1, S1>,
    engine_b: &mut ZamEngine<E2, P2, S2>,
) -> ZamResult<(usize, usize)>
where
    E1: EventStore,
    P1: PeerStore,
    S1: StateStore,
    E2: EventStore,
    P2: PeerStore,
    S2: StateStore,
{
    let vv_a = engine_a.replication_state().local_vv.clone();
    let vv_b = engine_b.replication_state().local_vv.clone();

    let mut applied_to_a = 0;
    for (node, start_seq) in vv_a.find_gaps(&vv_b) {
        for event in engine_b.events_since(node, start_seq)? {
            engine_a.apply_replicated(event)?;
            applied_to_a += 1;
        }
    }

    let mut applied_to_b = 0;
    for (node, start_seq) in vv_b.find_gaps(&vv_a) {
        for event in engine_a.events_since(node, start_seq)? {
            engine_b.apply_replicated(event)?;
            applied_to_b += 1;
        }
    }

    Ok((applied_to_a, applied_to_b))
}
