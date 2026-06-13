use zamsync_core::ports::{EventStore, PeerStore, StateStore, Transport};
use zamsync_core::{NodeId, SyncMessage, ZamError, ZamResult};

use crate::engine::ZamEngine;

#[derive(Debug, Default)]
pub struct SyncStats {
    pub events_sent: usize,
    pub events_received: usize,
}

pub struct SyncSession<'a, E, P, S, T>
where
    E: EventStore,
    P: PeerStore,
    S: StateStore,
    T: Transport,
{
    engine: &'a mut ZamEngine<E, P, S>,
    transport: &'a mut T,
}

impl<'a, E, P, S, T> SyncSession<'a, E, P, S, T>
where
    E: EventStore,
    P: PeerStore,
    S: StateStore,
    T: Transport,
{
    pub fn new(engine: &'a mut ZamEngine<E, P, S>, transport: &'a mut T) -> Self {
        Self { engine, transport }
    }

    /// Initiates a sync with `peer_id`: sends our handshake, processes the peer's
    /// response (handshake + event batches + SyncComplete), then sends our own events
    /// followed by SyncComplete.
    pub fn sync(&mut self, peer_id: NodeId) -> ZamResult<SyncStats> {
        let mut stats = SyncStats::default();

        self.transport
            .send(peer_id, &self.engine.prepare_handshake())?;

        let peer_vv = self.wait_for_handshake(peer_id)?;

        while let Some((from, msg)) = self.transport.receive()? {
            if from != peer_id {
                continue;
            }
            let is_complete = matches!(msg, SyncMessage::SyncComplete);
            if let SyncMessage::EventBatch { ref events, .. } = msg {
                stats.events_received += events.len();
            }
            self.engine.handle_sync_message(from, msg)?;
            if is_complete {
                break;
            }
        }

        let our_vv = self.engine.replication_state().local_vv.clone();
        let gaps = peer_vv.find_gaps(&our_vv);
        for (node, start_seq) in gaps {
            let events = self.engine.events_since(node, start_seq)?;
            if !events.is_empty() {
                stats.events_sent += events.len();
                self.transport.send(
                    peer_id,
                    &SyncMessage::EventBatch {
                        origin_node: node,
                        events,
                    },
                )?;
            }
        }
        self.transport.send(peer_id, &SyncMessage::SyncComplete)?;

        self.engine.sync()?;
        Ok(stats)
    }

    fn wait_for_handshake(
        &mut self,
        expected_peer: NodeId,
    ) -> ZamResult<zamsync_core::VersionVector> {
        for _ in 0..10_000 {
            match self.transport.receive()? {
                Some((from, SyncMessage::Handshake { vv, .. })) if from == expected_peer => {
                    return Ok(vv);
                }
                Some(_) | None => continue,
            }
        }
        Err(ZamError::Protocol(
            "timeout waiting for peer handshake".into(),
        ))
    }
}
