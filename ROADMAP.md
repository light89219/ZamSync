# ZamSync Roadmap

## 🟢 Phase 0-1: Foundation & Local Persistence (In Progress)
- [x] Workspace Initialization
- [x] Core Type Definitions (`SequenceNumber`, `NodeId`)
- [x] Production-Grade WAL (Atomic-like appends, CRC32, Recovery)
- [ ] Crash-Consistency Testing Suite

## 🟡 Phase 1: Event Model & State Machine
- [ ] Binary Event Schema (using `rkyv` for zero-copy)
- [ ] Local State Projection (Queryable view of the WAL)
- [ ] Engine Coordination (Applying WAL events to state)

## 🔴 Phase 2: Synchronization Protocol
- [ ] Version Vectors / HLC implementation
- [ ] Delta Sync Algorithm (Identifying missing event ranges)
- [ ] Conflict Resolution Strategy (LWW / CRDT-lite)

## 🔴 Phase 3: Resilient Transport
- [ ] Custom Binary Protocol (Optimized headers)
- [ ] Chunking & Resumable Transfer Mechanism
- [ ] Adaptive Backoff for unstable networks

## 🔴 Phase 4: Hardening & Performance
- [ ] Zstd Compression with pre-shared dictionaries
- [ ] End-to-End Encryption
- [ ] Resource profiling (< 100MB RAM target)
