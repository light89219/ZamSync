use zamsync_core::{Event, SequenceNumber, ZamResult};

/// The state projection represents the current "view" of the world.
/// It is updated by applying generic events from the WAL.
/// 
/// Implementation of this trait belongs to the application layer.
pub trait StateStore {
    /// Apply an event to the state. 
    /// The implementation must handle the `event_type` and `payload` according to its domain.
    fn apply_event(&mut self, seq: SequenceNumber, event: &Event) -> ZamResult<()>;
    
    /// Returns the last sequence number applied to this state.
    fn last_applied_seq(&self) -> Option<SequenceNumber>;
}
