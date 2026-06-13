use crate::adapters::{FilePeerStore, WalEventStore};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use zamsync_core::ports::{EventStore, PeerStore, StateStore};
use zamsync_core::{Event, Hlc, NodeId, ReplicationState, SequenceNumber, SyncMessage, ZamResult};

pub struct ZamEngine<E: EventStore, P: PeerStore, S: StateStore> {
    node_id: NodeId,
    event_store: E,
    peer_store: P,
    state: S,
    hlc: Hlc,
    replication: ReplicationState,
}

impl<E: EventStore, P: PeerStore, S: StateStore> ZamEngine<E, P, S> {
    pub fn new(node_id: NodeId, event_store: E, peer_store: P, mut state: S) -> ZamResult<Self> {
        let mut max_hlc = Hlc::default();

        for event_res in event_store.scan()? {
            let event = event_res?;
            if event.hlc > max_hlc {
                max_hlc = event.hlc;
            }
            state.apply_event(event.seq, &event)?;
        }

        let replication = peer_store.load()?;

        Ok(Self {
            node_id,
            event_store,
            peer_store,
            state,
            hlc: max_hlc,
            replication,
        })
    }

    pub fn submit(&mut self, event_type: u32, payload: Vec<u8>) -> ZamResult<SequenceNumber> {
        let now_ms = now_ms();
        self.hlc.tick(now_ms);
        let seq = self.event_store.next_seq();
        let event = Event {
            origin_node: self.node_id,
            seq,
            hlc: self.hlc,
            event_type,
            payload,
        };
        self.commit_event(event)
    }

    pub fn apply_replicated(&mut self, event: Event) -> ZamResult<SequenceNumber> {
        if let Some(&last) = self.replication.local_vv.entries.get(&event.origin_node.0) {
            if event.seq <= last {
                return Ok(event.seq);
            }
        }
        let now_ms = now_ms();
        self.hlc.sync(now_ms, &event.hlc);
        self.commit_event(event)
    }

    /// Returns all events from `origin_node` with `seq >= start_seq`.
    pub fn events_since(
        &self,
        origin_node: NodeId,
        start_seq: SequenceNumber,
    ) -> ZamResult<Vec<Event>> {
        let events = self
            .event_store
            .scan()?
            .filter_map(|r| r.ok())
            .filter(|e| e.origin_node == origin_node && e.seq.0 >= start_seq.0)
            .collect();
        Ok(events)
    }

    /// Builds a Handshake message from our current replication state.
    pub fn prepare_handshake(&self) -> SyncMessage {
        SyncMessage::Handshake {
            node_id: self.node_id,
            vv: self.replication.local_vv.clone(),
        }
    }

    /// Handles an incoming sync message and returns the response messages to send back.
    pub fn handle_sync_message(
        &mut self,
        from: NodeId,
        msg: SyncMessage,
    ) -> ZamResult<Vec<SyncMessage>> {
        match msg {
            SyncMessage::Handshake { vv, .. } => {
                let our_vv = self.replication.local_vv.clone();
                let gaps = vv.find_gaps(&our_vv);
                let mut responses = vec![self.prepare_handshake()];
                for (node, start_seq) in gaps {
                    let events = self.events_since(node, start_seq)?;
                    if !events.is_empty() {
                        responses.push(SyncMessage::EventBatch {
                            origin_node: node,
                            events,
                        });
                    }
                }
                responses.push(SyncMessage::SyncComplete);
                Ok(responses)
            }
            SyncMessage::PullRequest {
                origin_node,
                start_seq,
                limit,
            } => {
                let events = self
                    .events_since(origin_node, start_seq)?
                    .into_iter()
                    .take(limit as usize)
                    .collect();
                Ok(vec![SyncMessage::EventBatch {
                    origin_node,
                    events,
                }])
            }
            SyncMessage::EventBatch { events, .. } => {
                for event in events {
                    self.apply_replicated(event)?;
                }
                Ok(vec![])
            }
            SyncMessage::SyncComplete => {
                self.replication.peers.entry(from.0).or_default().known_vv =
                    self.replication.local_vv.clone();
                Ok(vec![])
            }
        }
    }

    pub fn scan_events(&self) -> ZamResult<Box<dyn Iterator<Item = ZamResult<Event>>>> {
        self.event_store.scan()
    }

    pub fn state(&self) -> &S {
        &self.state
    }

    pub fn node_id(&self) -> NodeId {
        self.node_id
    }

    pub fn replication_state(&self) -> &ReplicationState {
        &self.replication
    }

    pub fn sync(&mut self) -> ZamResult<()> {
        self.event_store.sync()?;
        self.peer_store.save(&self.replication)
    }

    fn commit_event(&mut self, event: Event) -> ZamResult<SequenceNumber> {
        let local_seq = self.event_store.append(&event)?;
        self.state.apply_event(local_seq, &event)?;
        self.replication
            .local_vv
            .update(event.origin_node, event.seq);
        Ok(local_seq)
    }
}

impl<S: StateStore> ZamEngine<WalEventStore, FilePeerStore, S> {
    pub fn open_wal(data_dir: impl AsRef<Path>, node_id: NodeId, state: S) -> ZamResult<Self> {
        let dir = data_dir.as_ref();
        let event_store = WalEventStore::open(dir.join("events.wal"))?;
        let peer_store = FilePeerStore::open(dir.join("peers.state"), node_id)?;
        ZamEngine::new(node_id, event_store, peer_store, state)
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
