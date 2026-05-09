use zamsync_core::{Event, ZamResult};
use std::collections::BinaryHeap;
use std::cmp::Ordering;

/// LogSorter is a mechanical merger that combines multiple event streams.
/// It ensures that the output stream follows the deterministic global order rule.
pub struct LogSorter<I> 
where 
    I: Iterator<Item = ZamResult<Event>>
{
    sources: Vec<std::iter::Peekable<I>>,
    heap: BinaryHeap<IndexedEvent>,
}

struct IndexedEvent {
    event: Event,
    source_idx: usize,
}

impl PartialEq for IndexedEvent {
    fn eq(&self, other: &Self) -> bool {
        self.event.hlc == other.event.hlc && self.event.origin_node == other.event.origin_node
    }
}

impl Eq for IndexedEvent {}

impl PartialOrd for IndexedEvent {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for IndexedEvent {
    fn cmp(&self, other: &Self) -> Ordering {
        // Min-Heap based on (HLC, NodeId)
        match self.event.hlc.cmp(&other.event.hlc) {
            Ordering::Equal => other.event.origin_node.0.cmp(&self.event.origin_node.0),
            other_order => other_order.reverse(),
        }
    }
}

impl<I> LogSorter<I> 
where 
    I: Iterator<Item = ZamResult<Event>>
{
    pub fn new(iterators: Vec<I>) -> ZamResult<Self> {
        let mut sources: Vec<_> = iterators.into_iter().map(|it| it.peekable()).collect();
        let mut heap = BinaryHeap::new();

        // Initial fill of the heap
        for (idx, source) in sources.iter_mut().enumerate() {
            if let Some(res) = source.next() {
                let event = res?;
                heap.push(IndexedEvent { event, source_idx: idx });
            }
        }

        Ok(Self { sources, heap })
    }
}

impl<I> Iterator for LogSorter<I> 
where 
    I: Iterator<Item = ZamResult<Event>>
{
    type Item = ZamResult<Event>;

    fn next(&mut self) -> Option<Self::Item> {
        let IndexedEvent { event, source_idx } = self.heap.pop()?;

        // Refill from the same source
        if let Some(res) = self.sources[source_idx].next() {
            match res {
                Ok(next_event) => {
                    self.heap.push(IndexedEvent { event: next_event, source_idx });
                }
                Err(e) => return Some(Err(e)),
            }
        }

        Some(Ok(event))
    }
}
