# ZamSync — Resilient Offline-First Synchronization Engine

ZamSync is a systems-level synchronization engine designed for environments where network connectivity is unreliable, intermittent, or extremely low-bandwidth.

It targets real-world infrastructure constraints observed in rural and mountainous regions where traditional cloud-first or HTTP-based synchronization systems fail under network instability.

---

## Context and Motivation

Reliable digital infrastructure is still a major challenge in remote regions worldwide.

In countries with complex geography such as Bhutan, large parts of the territory are composed of high-altitude terrain, making connectivity inconsistent across villages, clinics, and administrative centers.

Public digital transformation initiatives such as national e-health systems (e.g., electronic patient record systems like ePIS) face recurring challenges:

- unstable or intermittent connectivity
- high latency links (2G / satellite / constrained mobile networks)
- frequent disconnections during data synchronization
- fallback to manual or paper-based workflows during outages

This leads to a fundamental issue:

> Most modern synchronization systems assume stable connectivity, which does not hold in these environments.

---

## Problem Statement

Traditional approaches (REST APIs, JSON-based synchronization, cloud-first architectures) introduce:

- high protocol overhead
- inefficient bandwidth usage
- poor resilience to disconnections
- inability to resume transfers at fine granularity
- strong dependency on persistent connectivity

ZamSync is designed to operate under the opposite assumption:

> The network is unreliable by default.

---

## Design Goals

ZamSync aims to provide a synchronization layer that is:

- offline-first by design
- resilient to frequent and unpredictable disconnections
- highly bandwidth-efficient
- deterministic and replay-safe
- capable of exact resumability after failure
- suitable for low-resource devices in constrained environments

---

## System Architecture

### 1. Storage Layer

- Local persistent database (SQLite or embedded key-value store)
- Write-Ahead Log (WAL) as the source of truth
- Append-only event model
- State reconstruction from deterministic event replay

---

### 2. Synchronization Layer

- Event-based replication model
- Sequence-based or version-based diff detection
- Incremental synchronization using missing event ranges
- Idempotent application of events across nodes

---

### 3. Transport Layer

- Lightweight binary protocol optimized for minimal overhead
- Chunk-based transfer system for large payloads
- Explicit acknowledgment mechanism (range or bitmap-based)
- Full resumability after interruption without retransfer

---

### 4. Serialization Layer

- Compact binary encoding (varint-based structures)
- Optional compression layer (e.g. zstd)
- Schema-driven event representation
- Optional dictionary encoding for frequent domain terms

---

## Failure Model

The system is explicitly designed to tolerate:

- frequent network disconnections
- high packet loss rates
- long offline periods
- partial transfers
- corrupted or incomplete transmissions

All operations are retry-safe and idempotent by design.

---

## Data Integrity Model

ZamSync enforces:

- per-chunk checksum validation
- event-level integrity verification
- replay-safe event application
- explicit detection of corruption or incomplete state

No silent data loss is permitted under any failure scenario.

---

## Testing Strategy

Every component must be validated under realistic conditions.

### Unit Testing
- serialization correctness
- WAL consistency
- event validation logic
- edge-case input handling

### Integration Testing
- storage and synchronization interaction
- transport and recovery behavior

### Failure Simulation
- high latency networks
- packet loss up to extreme levels
- sudden disconnections
- corrupted data streams

### Determinism Testing
- identical state reconstruction across repeated runs
- consistent sync outcomes across nodes

---

## Benchmarking Requirements

All implementations must be measured against baseline approaches such as:

- REST/JSON synchronization
- naive full-state replication
- unoptimized file transfer systems

Key metrics:

- bandwidth usage
- synchronization latency
- recovery time after failure
- memory footprint under constrained hardware
- success rate under simulated network degradation

---

## Non-Goals

ZamSync explicitly avoids:

- blockchain-based consensus systems
- cloud-first architectural dependency
- heavy distributed coordination frameworks
- unnecessary microservices complexity
- assumptions of stable connectivity

---

## Long-Term Vision

ZamSync is intended as a foundation for:

- offline-first distributed systems
- resilient synchronization in infrastructure-limited environments
- low-bandwidth critical data replication systems
- minimal alternatives to cloud-dependent synchronization tools

The objective is to provide a predictable, deterministic, and robust synchronization engine that operates reliably where conventional systems fail.