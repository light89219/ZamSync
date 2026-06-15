# Acknowledgements

ZamSync did not emerge from a vacuum. Every design decision -- from the WAL
format to the sync protocol to the wire encoding -- was informed by decades of
prior work in distributed systems research, the extraordinary Rust ecosystem,
and the accumulated wisdom of open-source communities worldwide. This file is an
attempt to name that debt honestly and completely.

---

## Academic Foundations

The theoretical core of ZamSync rests on a small number of landmark papers.
Anyone who wants to understand *why* the engine is built the way it is should
read these first.

**Leslie Lamport -- "Time, Clocks, and the Ordering of Events in a Distributed
System" (1978)**
The paper that introduced logical clocks. ZamSync's HLC is a direct descendant
of Lamport's insight that message-passing systems can establish a consistent
global ordering without synchronized physical clocks.
<https://lamport.azurewebsites.net/pubs/time-clocks.pdf>

**Sandeep Kulkarni, Murat Demirbas, Deepak Madappa, Bharadwaj Avva, Marcelo
Leone -- "Logical Physical Clocks and Consistent Snapshots in Globally
Distributed Databases" (2014)**
The Hybrid Logical Clock (HLC) paper. ZamSync uses HLC directly: every event
carries an HLC timestamp that is both causally consistent and within a bounded
offset of wall-clock time. The `Hlc` struct and its monotonicity guarantees are
a faithful implementation of this paper's Definition 3 and Algorithm 5.
<https://cse.buffalo.edu/tech-reports/2014-04.pdf>

**Colin Fidge -- "Timestamps in Message-Passing Systems That Preserve the
Partial Ordering" (1988)**
Independent development of vector clocks. ZamSync's `VersionVector` is a
classical vector clock in the Fidge/Mattern tradition, used to track per-node
sequence progress and compute sync gaps.
<https://doi.org/10.1145/3149.214121>

**Friedemann Mattern -- "Virtual Time and Global States of Distributed Systems"
(1988)**
Parallel development of vector clocks with Fidge. Mattern's formulation of the
"consistent cut" is what makes ZamSync's `find_gaps` semantics correct: a gap
is only a gap relative to what a node has seen from a given peer.

**Giuseppe DeCandia, Deniz Hastorun, Madan Jampani, Gunavardhan Kakulapati,
Avinash Lakshman, Alex Pilchin, Swaminathan Sivasubramanian, Peter Vosshall,
Werner Vogels -- "Dynamo: Amazon's Highly Available Key-value Store" (2007)**
The paper that demonstrated version vectors at production scale. ZamSync's
approach to eventual consistency and conflict detection follows Dynamo's
philosophy: prefer availability, track causality via vectors, surface conflicts
to the application.
<https://dl.acm.org/doi/10.1145/1294261.1294281>

**Marc Shapiro, Nuno Preguica, Carlos Baquero, Marek Zawirski -- "Conflict-free
Replicated Data Types" (2011)**
CRDTs clarified the design space for convergent distributed data structures.
ZamSync is not a CRDT system, but the CRDT literature informed the decision to
use an append-only, monotonically-growing event log rather than a mutable
state store: append-only logs converge trivially.
<https://link.springer.com/chapter/10.1007/978-3-642-24550-3_29>

**Peter Bailis, Ali Ghodsi -- "Eventual Consistency Today: Limitations,
Extensions, and Beyond" (2013)**
A rigorous treatment of what "eventual consistency" actually guarantees and
where it falls short. This paper shaped ZamSync's access control design:
eventual consistency is safe for replication but unsafe for access control,
which must be enforced at serve time rather than at application time.
<https://dl.acm.org/doi/10.1145/2445388.2445401>

---

## The Rust Ecosystem

ZamSync is written in Rust. The following crates are direct dependencies; their
authors and contributors did the hard work so this project could focus on the
sync protocol rather than reinventing low-level primitives.

### Serialization and encoding

**[rkyv](https://github.com/rkyv/rkyv)** -- David Koloski and contributors
Zero-copy deserialization. Every WAL record and every sync message is serialized
with rkyv, which means the bytes written to disk are the same bytes the decoder
reads -- no allocation, no copying. The `SyncMessage` and `Event` types live in
rkyv's archive format over the wire.

**[serde](https://github.com/serde-rs/serde)** -- Erick Tryzelaar, David Tolnay
and contributors
The de-facto serialization framework for Rust. ZamSync uses serde for JSON
payload validation and the HTTP API layer.

**[serde_json](https://github.com/serde-rs/json)** -- David Tolnay
JSON serialization and deserialization built on serde. Used for the REST API
response types, audit log JSON output, and payload schema validation.

**[bytecheck](https://github.com/rkyv/bytecheck)** -- David Koloski
Byte-level validation for rkyv archives. Ensures that malformed or truncated
network frames are caught before deserialization, preventing undefined behavior
from corrupted wire data.

**[byteorder](https://github.com/BurntSushi/byteorder)** -- Andrew Gallant
(BurntSushi)
Deterministic big-endian and little-endian integer encoding. ZamSync's wire
protocol uses a 4-byte big-endian length prefix on every frame; `byteorder`
makes the encoding portable and explicit.

### Cryptography

**[chacha20poly1305](https://github.com/RustCrypto/AEADs)** -- RustCrypto
contributors
ChaCha20-Poly1305 AEAD encryption. Every WAL record is encrypted at rest with a
random 96-bit nonce and a 256-bit key derived from the user's key file.
ChaCha20-Poly1305 was chosen over AES-GCM for its constant-time guarantees on
hardware without AES acceleration -- critical for ARM Cortex-A7 nodes (Raspberry
Pi 2/3) that lack the `ARMv8-A` AES extension.

**[rustls](https://github.com/rustls/rustls)** -- Joseph Birr-Pixton, Daniel
McCarney, Dirkjan Ochtman, and the rustls contributors
Pure-Rust TLS 1.3 implementation. ZamSync uses rustls for mutual TLS (mTLS)
between every hub-clinic pair. Rustls was chosen over OpenSSL because it
compiles to musl-static without system library dependencies, making single-file
deployment on a fresh Raspberry Pi possible.

**[rcgen](https://github.com/rustls/rcgen)** -- est42 and contributors
Pure-Rust X.509 certificate generation. `zamsync keygen` and `zamsync sign` use
rcgen to produce Ed25519 CA and node certificates at runtime, with no dependency
on OpenSSL's `openssl` command-line tool.

**[rustls-pemfile](https://github.com/rustls/rustls-pemfile)** -- the rustls
contributors
PEM file parsing for rustls. Handles loading hub CA certificates and node keys
from the data directory.

**[sha2](https://github.com/RustCrypto/hashes)** -- RustCrypto contributors
SHA-256 hash function. Used in `zamsync audit` to produce a per-event integrity
hash that allows offline verification of the audit log without decrypting the
WAL.

**[crc32fast](https://github.com/srijs/rust-crc32fast)** -- Sam Rijs, Alex
Crichton
SIMD-accelerated CRC32 checksums. Every WAL record carries a CRC32 over its
header and payload; on startup ZamSync walks the WAL, verifying each checksum
and truncating at the first failed record -- the mechanism that gives
crash-consistent recovery.

### Compression

**[zstd](https://github.com/gyscos/zstd-rs)** -- Alexandre Bury
Rust bindings for the Zstandard compression library created by Yann Collet at
Facebook. Every sync frame larger than 64 bytes is compressed at level 3 before
transmission; the compressor transparently falls back to the raw frame if
compression would increase the size (common for already-compressed payloads).
Zstd's dictionary-training capability is the basis for Phase 18's satellite
compression profile.

### Async I/O and networking

**[tokio](https://github.com/tokio-rs/tokio)** -- Carl Lerche, Alice Ryhl, and
the Tokio contributors
The async runtime underlying the HTTP server. ZamSync's core sync engine is
intentionally synchronous (suitable for `no_std` and embedded targets), but the
REST API layer uses tokio's single-threaded runtime in a dedicated OS thread,
giving async I/O without making the engine itself async.

**[axum](https://github.com/tokio-rs/axum)** -- David Pedersen and the Tokio
contributors
Ergonomic, macro-free HTTP framework built on Hyper and Tower. ZamSync's
embedded REST API (`POST /submit`, `GET /events`, `GET /events/stream`,
`GET /health`) is implemented with axum, including the SSE stream endpoint used
by real-time frontends.

**[tokio-stream](https://github.com/tokio-rs/tokio-stream)** -- the Tokio
contributors
Stream utilities for tokio. Used in the SSE endpoint to convert an `unfold`-
based polling stream into a proper `Stream` that axum can serve.

**[futures](https://github.com/rust-lang/futures-rs)** -- Alex Crichton and
contributors
Core async primitives for Rust. The `StreamExt::flat_map` combinator used in
the SSE handler comes from this crate.

### Observability

**[tracing](https://github.com/tokio-rs/tracing)** -- Eliza Weisman and the
Tokio contributors
Structured, context-aware diagnostic instrumentation. Every sync session emits
tracing spans that carry the peer node ID, making `RUST_LOG=zamsync=debug`
output navigable even with 16 concurrent peers.

**[tracing-subscriber](https://github.com/tokio-rs/tracing-subscriber)** -- the
Tokio contributors
Subscriber implementations for `tracing`. ZamSync uses the `EnvFilter` layer to
respect the `RUST_LOG` environment variable and the `fmt` layer for human-
readable output in development and structured JSON in production deployments.

**[metrics](https://github.com/metrics-rs/metrics)** -- Toby Lawrence and
contributors
A lightweight, zero-allocation metrics façade for Rust. ZamSync instruments
`events_submitted_total`, `sync_duration_seconds`, `events_sent_total`,
`events_received_total`, and `version_vector_drift` behind the metrics API so
the backend (Prometheus, StatsD, etc.) is swappable without touching engine code.

**[metrics-exporter-prometheus](https://github.com/metrics-rs/metrics)** -- the
metrics-rs contributors
Prometheus-compatible exporter for the metrics façade. Exposes a `/metrics`
scrape endpoint compatible with any Prometheus-based monitoring stack -- Grafana,
VictoriaMetrics, Thanos.

### Storage and data

**[rusqlite](https://github.com/rusqlite/rusqlite)** -- The rusqlite
contributors
Safe, ergonomic Rust bindings for SQLite. ZamSync bundles SQLite via the
`bundled` feature so the projection service (`zamsync project`) runs on any
target without requiring a system SQLite installation. The projection schema uses
`INSERT OR IGNORE` on a `UNIQUE(origin_node_id, seq)` constraint for idempotent,
resumable projection.

### Utilities

**[thiserror](https://github.com/dtolnay/thiserror)** -- David Tolnay
Derive macro for the `std::error::Error` trait. ZamSync's `ZamError` enum is
derived with `thiserror`, keeping error variant definitions concise and ensuring
`Display` and `source` impls stay in sync with the type.

**[tempfile](https://github.com/Stebalien/tempfile)** -- Steven Allen
Temporary file and directory management with guaranteed cleanup on drop. Every
unit and integration test that touches the filesystem creates a `tempdir()` --
no global test state, no cross-test interference.

**[log](https://github.com/rust-lang/log)** -- The Rust project
The standard Rust logging façade. Used in library crates where pulling tokio's
`tracing` as a hard dependency would be inappropriate; the tracing-log bridge
routes these records into the tracing subscriber in binary crates.

**[windows-sys](https://github.com/microsoft/windows-rs)** -- Microsoft
Raw Windows API bindings generated from the Windows metadata. ZamSync uses
`Win32_System_ProcessStatus` to read RSS memory usage in `zamsync bench` on
Windows, mirroring the `/proc/self/status` read used on Linux.

**[time](https://github.com/time-rs/time)** -- Jacob Pratt and contributors
Date and time utilities used in the network test suite for constructing
deliberately expired X.509 certificates to verify that rustls rejects them at
the TLS handshake.

---

## Projects That Shaped the Design

These projects were not dependencies but design references -- their source code,
documentation, and architecture were studied during ZamSync's design phase.

**[SQLite](https://sqlite.org)** -- D. Richard Hipp and the SQLite team
The WAL design in ZamSync was directly inspired by SQLite's WAL mode. The
concepts of an append-only write-ahead log, a checksum per record, a tombstone
for compacted regions, and crash-consistent recovery via truncation at the first
bad record all appear in SQLite's file format documentation and influenced
ZamSync's WAL format decisions.

**[PostgreSQL](https://www.postgresql.org)** -- The PostgreSQL Global Development
Group
PostgreSQL's WAL design, MVCC model, and replication protocol documentation
provided a production-scale reference for write-ahead logging. The PostgreSQL
wiki's coverage of WAL internals is among the clearest engineering writing on
the subject.

**[CouchDB](https://couchdb.apache.org)** -- The Apache Software Foundation
CouchDB pioneered offline-first database replication in the open-source world.
Its revision-based conflict model and the "replication is just HTTP" philosophy
informed ZamSync's decision to make sync a first-class protocol primitive rather
than a bolt-on feature.

**[PouchDB](https://pouchdb.com)** -- Dale Harvey and contributors
The browser-side CouchDB-compatible sync engine. PouchDB's "sync adapters"
pattern -- abstract the storage backend, keep the sync protocol stable --
directly influenced ZamSync's hexagonal architecture (pluggable `EventStore`,
`PeerStore`, `Transport` adapters behind a stable `ZamEngine` core).

**[Automerge](https://automerge.org)** -- Martin Kleppmann, Orion Henry,
Alex Good, and contributors
A CRDT-based collaborative editing library. Automerge's approach to conflict
resolution as data (conflicts are first-class values, not errors) is the
philosophical basis for ZamSync's Phase 17 conflict visibility design: conflicts
are emitted as new WAL events, preserving full audit history.

**[Ditto](https://ditto.live)** -- Ditto Inc.
A commercial offline-first sync engine for mobile. Ditto's public writing on
mesh networking and peer discovery informed Phase 16's mDNS design.

**[etcd](https://etcd.io)** -- The etcd authors, CNCF
The reference implementation of Raft consensus in Go. ZamSync does not use Raft
(it uses a simpler append-only protocol appropriate for its eventual-consistency
model), but etcd's source code is the clearest available implementation of
distributed consensus concepts.

**[TiKV](https://tikv.org)** -- The TiKV authors, CNCF
A distributed transactional key-value store in Rust. TiKV demonstrated that
production-grade distributed storage is achievable in Rust and provided
reference patterns for WAL design, RocksDB integration, and testing strategy.

**[Riak](https://github.com/basho/riak)** -- Basho Technologies
Riak's production deployment of vector clocks at scale shaped ZamSync's
`VersionVector` implementation. Riak's engineering blog posts on vector clock
pruning and dotted version vectors are essential reading for anyone working in
this space.

**[rsync](https://rsync.samba.org)** -- Andrew Tridgell and Paul Mackerras
The original delta-sync algorithm. ZamSync's sync protocol is not delta-based
(events are the delta), but rsync's framing of "what does the receiver already
have?" as the central sync question is the same question that version vectors
answer.

**[Toxiproxy](https://github.com/Shopify/toxiproxy)** -- The Shopify platform
engineering team
A TCP proxy for simulating network conditions in tests. ZamSync's entire Phase
13 field simulation -- 2G at 600ms/30 kbps, satellite at 1200ms/100 kbps, mid-
sync connection cuts -- is built on Toxiproxy. Without it, testing resilience
under realistic conditions would require actual hardware or complex kernel-level
traffic shaping.

**[VHS](https://github.com/charmbracelet/vhs)** -- the charmbracelet team
A terminal recorder that turns `.tape` scripts into animated GIFs. All demo GIFs
in `docs/demos/` were produced with VHS, enabling repeatable, reviewable
terminal recordings without a screen-capture dependency.

**[IPFS](https://ipfs.tech)** -- Protocol Labs
IPFS appears in ZamSync's benchmark report as the primary comparison target.
Studying IPFS's architecture clarified the design space: IPFS is a content-
addressed filesystem optimized for public data distribution, not a sync engine
for private, ordered, encrypted event streams. ZamSync is not a replacement for
IPFS; it is a different tool for a different problem.

**[cross](https://github.com/cross-rs/cross)** -- Jorge Aparicio and the
cross-rs contributors
Zero-configuration cross-compilation for Rust using Docker. The ARM targets in
ZamSync's CI (`aarch64-unknown-linux-musl`, `armv7-unknown-linux-musleabihf`)
are built with `cross` on x86_64 GitHub Actions runners, producing fully static
binaries that run on any Raspberry Pi model.

---

## Standards and Specifications

The following IETF RFCs and open standards are load-bearing in ZamSync's
security and interoperability model.

**RFC 8446 -- The Transport Layer Security (TLS) Protocol Version 1.3** (2018)
TLS 1.3 is the only TLS version accepted by ZamSync's mTLS transport. The
protocol eliminates legacy cipher suites and reduces the handshake to a single
round trip, which matters on high-latency satellite links.
<https://datatracker.ietf.org/doc/html/rfc8446>

**RFC 7539 -- ChaCha20 and Poly1305 for IETF Protocols** (2015)
The normative specification for the AEAD cipher used to encrypt every WAL
record. RFC 7539 defines the nonce construction and authentication tag format
that `chacha20poly1305` implements.
<https://datatracker.ietf.org/doc/html/rfc7539>

**RFC 8032 -- Edwards-Curve Digital Signature Algorithm (EdDSA)** (2017)
The Ed25519 signature scheme used for X.509 node certificates. `rcgen` generates
Ed25519 keys; rustls verifies them at the mTLS handshake.
<https://datatracker.ietf.org/doc/html/rfc8032>

**RFC 5280 -- Internet X.509 Public Key Infrastructure Certificate and CRL
Profile** (2008)
The X.509 certificate format used by `zamsync keygen` and `zamsync sign`.
The hub CA signs clinic node certificates according to this profile; rustls
validates the chain at TLS handshake time.
<https://datatracker.ietf.org/doc/html/rfc5280>

**ISO 8601 -- Date and Time Format**
All timestamps in `zamsync audit` output are ISO 8601 UTC strings
(`2024-03-15T09:32:11Z`). Machine-readable, sortable, unambiguous across time
zones -- the only sensible choice for a system deployed across multiple sites.

**The Prometheus Data Model** -- CNCF
ZamSync's metrics endpoint (`/metrics`) is compatible with the Prometheus
exposition format. The Prometheus data model (counters, gauges, histograms,
labels) shaped how ZamSync's instrumentation is structured.
<https://prometheus.io/docs/concepts/data_model/>

**NO_COLOR** -- Xe Iaso (and the open specification)
A community standard for disabling ANSI color output in CLI tools. ZamSync
respects the `NO_COLOR` environment variable: when set, all terminal color codes
are suppressed regardless of TTY detection.
<https://no-color.org>

---

## Books and Learning Resources

These texts were formative references during ZamSync's design and implementation.

**Martin Kleppmann -- "Designing Data-Intensive Applications" (2017)**
O'Reilly Media. The single most useful book for anyone building a distributed
storage system. Chapters 5 (replication), 8 (distributed systems problems), and
9 (consistency and consensus) map almost directly to ZamSync's design
challenges. The discussion of version vectors, partial failures, and the
difference between availability and consistency shaped every major design
decision in this project.

**Alex Petrov -- "Database Internals: A Deep Dive into How Distributed Data
Systems Work" (2019)**
O'Reilly Media. Part I (storage engines, B-trees, LSM trees, WAL) is the best
available treatment of how databases actually persist data. The WAL chapter
informed ZamSync's record format, CRC placement, and recovery procedure.

**Steve Klabnik and Carol Nichols -- "The Rust Programming Language" (2nd ed.)**
The Rust Foundation. The official Rust book. Required reading; the ownership
model, trait system, and lifetime rules that make ZamSync's hexagonal
architecture safe without runtime overhead are explained here.
<https://doc.rust-lang.org/book/>

**Jon Gjengset -- "Rust for Rustaceans" (2021)**
No Starch Press. The bridge between knowing Rust and writing idiomatic,
production-quality Rust. The chapters on advanced trait usage, API design, and
testing shaped how ZamSync's port traits (`EventStore`, `PeerStore`, etc.) are
defined and the testing adapters are structured.

**Jim Blandy, Jason Orendorff, Leonora F.S. Tindall -- "Programming Rust" (2nd
ed., 2021)**
O'Reilly Media. Deep coverage of Rust's type system, concurrency model, and
unsafe code. Essential reference during the implementation of the WAL writer and
the zero-copy frame buffer.

---

## Architecture Patterns

**Alistair Cockburn -- Hexagonal Architecture (Ports and Adapters) (2005)**
ZamSync's core architecture is a direct implementation of the Ports and Adapters
pattern. The sync engine (`ZamEngine`) depends only on abstract port traits
(`EventStore`, `PeerStore`, `StateStore`, `Transport`); concrete adapters
(WAL-backed storage, TCP transport, in-memory test adapters) are injected at
construction. This made it possible to test the entire sync protocol against
in-memory adapters before writing a single line of TCP code.
<https://alistair.cockburn.us/hexagonal-architecture/>

**Martin Fowler -- Event Sourcing (2005)**
ZamSync is an event-sourced system: the WAL is the source of truth, and current
state is always derived by replaying events. Fowler's canonical description of
event sourcing informed the decision to make events immutable and the WAL
append-only.
<https://martinfowler.com/eaaDev/EventSourcing.html>

**Martin Fowler -- CQRS (2011)**
The separation between the write path (`submit` → WAL → sync) and the read path
(`zamsync project` → SQLite → application queries) follows the CQRS pattern.
<https://martinfowler.com/bliki/CQRS.html>

---

## Health Information Systems Context

ZamSync's reference deployment scenario -- offline-first synchronization for
rural health clinics -- was shaped by the following systems and standards.

**[DHIS2](https://dhis2.org)** -- University of Oslo and the DHIS2 community
The world's largest health management information system, deployed in 73
countries. DHIS2's offline-capable Android app and its data synchronization
challenges are the direct motivation for ZamSync's Bhutan ePIS reference
scenario. The DHIS2 architecture documentation clarified what "offline-first
health data" means in practice.

**[OpenMRS](https://openmrs.org)** -- The OpenMRS community
An open-source electronic medical record system widely deployed in low-resource
settings. OpenMRS's Sync 2.0 module and its documented failure modes on
intermittent connections provided concrete use-case evidence for ZamSync's design
goals.

**[WHO SMART Guidelines](https://www.who.int/teams/digital-health-and-innovation/smart-guidelines)**
The World Health Organization's framework for digital clinical guidelines.
The SMART Guidelines' emphasis on interoperability, data sovereignty, and
deployability in low-resource environments aligns with ZamSync's design
principles.

**[Bhutan Ministry of Health -- eHealth Strategy](https://www.health.gov.bt)**
The Bhutan Ministry of Health's digital health roadmap, including the ePIS
(electronic patient information system) initiative, is the named reference
deployment target in ZamSync's documentation. The connectivity constraints
documented for Bhutan's rural clinics -- 2G EDGE at best, VSAT with per-MB
billing, multi-hour offline periods -- are the basis for the Toxiproxy network
profiles used in the field simulation.

---

## The Rust Community

**[crates.io](https://crates.io)** and **[docs.rs](https://docs.rs)**
The Rust package registry and documentation hosting platform. Every dependency
listed above was published here. The quality of documentation on docs.rs -- in
particular the rustls, rkyv, and chacha20poly1305 crates -- made integration
substantially faster.

**[users.rust-lang.org](https://users.rust-lang.org)**
The Rust users forum. Several design decisions in ZamSync (the rkyv validation
strategy, the async-in-a-thread pattern for the HTTP server, the musl static
build configuration) were informed by forum discussions.

**[The Rust Discord](https://discord.gg/rust-lang)**
The official Rust community Discord. The `#help` and `#async` channels provided
timely answers during the implementation of the SSE streaming endpoint and the
Tokio integration.

**[r/rust](https://www.reddit.com/r/rust/)**
The Rust subreddit. A consistent source of real-world experience reports on
Rust in production -- particularly useful for understanding the tradeoffs between
synchronous and asynchronous Rust at ZamSync's scale.

**[This Week in Rust](https://this-week-in-rust.org)**
Weekly Rust newsletter curated by volunteers. An invaluable resource for staying
current with the ecosystem -- several of ZamSync's dependency choices were
informed by crate announcements and community discussions surfaced here.

---

*ZamSync is MIT-licensed. All dependencies are used in accordance with their
respective licenses. A full machine-readable list of transitive dependencies and
their license identifiers is available by running `cargo license` in the
repository root.*
