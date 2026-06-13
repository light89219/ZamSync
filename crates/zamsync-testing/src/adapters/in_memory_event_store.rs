use std::collections::VecDeque;
use zamsync_core::ports::EventStore;
use zamsync_core::{Event, SequenceNumber, ZamResult};

#[derive(Default, Clone)]
pub struct InMemoryEventStore {
    events: Vec<Event>,
}

impl InMemoryEventStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn events(&self) -> &[Event] {
        &self.events
    }
}

impl EventStore for InMemoryEventStore {
    fn next_seq(&self) -> SequenceNumber {
        SequenceNumber(self.events.len() as u64)
    }

    fn append(&mut self, event: &Event) -> ZamResult<SequenceNumber> {
        let seq = self.next_seq();
        self.events.push(event.clone());
        Ok(seq)
    }

    fn scan(&self) -> ZamResult<Box<dyn Iterator<Item = ZamResult<Event>>>> {
        let events = self.events.clone();
        Ok(Box::new(events.into_iter().map(Ok)))
    }

    fn sync(&mut self) -> ZamResult<()> {
        Ok(())
    }
}

pub struct InMemoryEventCursor {
    events: VecDeque<Event>,
}

impl InMemoryEventCursor {
    pub fn from_store(store: &InMemoryEventStore) -> Self {
        Self {
            events: store.events.iter().cloned().collect(),
        }
    }
}

impl Iterator for InMemoryEventCursor {
    type Item = ZamResult<Event>;

    fn next(&mut self) -> Option<Self::Item> {
        self.events.pop_front().map(Ok)
    }
}
