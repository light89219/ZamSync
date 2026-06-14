<p align="center">
  <h1 align="center">ZamSync</h1>
  <p align="center"><strong>Offline-first synchronization engine for intermittent-connectivity deployments</strong></p>
  <p align="center">
    Deterministic WAL replication &bull; mTLS PKI &bull; WAL encryption &bull; ARM64 / ARMv7 native
  </p>
</p>

<p align="center">
  <img src="https://img.shields.io/github/actions/workflow/status/Etoile-Bleu/ZamSync/ci.yml?branch=main&label=CI&style=flat-square" alt="CI status">
  <img src="https://img.shields.io/github/v/release/Etoile-Bleu/ZamSync?style=flat-square&label=release" alt="Latest release">
  <img src="https://img.shields.io/badge/license-MIT-blue?style=flat-square" alt="MIT license">
  <img src="https://img.shields.io/badge/rust-1.75%2B-orange?style=flat-square" alt="Rust 1.75+">
  <img src="https://img.shields.io/badge/platforms-linux%20%7C%20arm64%20%7C%20armv7%20%7C%20windows-lightgrey?style=flat-square" alt="Platforms">
  <img src="https://img.shields.io/badge/docker-ghcr.io-2496ED?style=flat-square&logo=docker" alt="Docker">
</p>

---

## What is ZamSync?

ZamSync is a systems-level synchronization engine for environments where network connectivity is unreliable, intermittent, or extremely low-bandwidth -- think 2G links, satellite connections, or Raspberry Pi nodes in remote clinics.

It provides **deterministic, bidirectional event replication** between nodes, designed to work correctly through:

- hours or days of complete disconnection
- mid-transfer connection cuts
- duplicate deliveries and partial writes
- power loss during a sync session

The reference deployment is the **Bhutan ePIS** (electronic patient information system), where district clinics sync patient records to a central hospital hub over 2G cellular with 600ms latency and frequent blackouts.

---

## Features

| Category | Feature |
|---|---|
| **Core** | Append-only WAL with CRC32 integrity and crash recovery |
| **Replication** | Version Vector deduplication -- no duplicate events ever |
| **Ordering** | Hybrid Logical Clocks (HLC) for total deterministic ordering |
| **Security** | Mutual TLS (mTLS) with a hub-signed PKI -- rogue nodes rejected at handshake |
| **Encryption** | WAL encryption at rest (ChaCha20-Poly1305), per-record random nonce |
| **Access control** | `--policy own` -- clinic A cannot read clinic B's records |
| **Validation** | JSON schema enforcement on submit and replicated events |
| **Observability** | Prometheus metrics, structured tracing, audit trail (JSON Lines) |
| **Operations** | WAL compaction, key rotation, daemon mode, benchmarking |
| **Efficiency** | Zstd level-3 compression on all frames, chunked batches (256 events/frame) |
| **Platforms** | x86_64-linux, aarch64-linux, armv7-linux, x86_64-windows |

---

## Architecture

ZamSync uses **hexagonal architecture** -- the sync core is pure logic with no I/O. Storage and transport are pluggable adapters.

```
  ┌─────────────────────────────────────────────────────────┐
  │                    zamsync (CLI binary)                  │
  └───────────────────────┬─────────────────────────────────┘
                          │
          ┌───────────────┼───────────────┐
          │               │               │
  ┌───────▼──────┐ ┌──────▼──────┐ ┌─────▼──────────┐
  │ zamsync-core │ │zamsync-stor-│ │zamsync-network  │
  │              │ │age          │ │                 │
  │ Event        │ │ ZamEngine   │ │ TcpTransport    │
  │ HLC          │ │ WalEventSt. │ │ TlsTcpTransport │
  │ VersionVect. │ │ FilePeerSt. │ │ wire protocol   │
  │ SyncMessage  │ │ SyncSession │ │ mTLS (rustls)   │
  │ port traits  │ │ WAL (CRC32) │ │                 │
  └──────────────┘ └─────────────┘ └─────────────────┘
```

### Sync protocol

Two nodes exchange `SyncMessage` frames over a TCP (or TLS) stream:

```
  Clinic (initiator)              Hospital (responder)
       │                                  │
       │── Handshake(node_id, vv) ───────>│
       │                                  │  compare VVs, find gaps
       │<── EventBatch(events...) ────────│
       │<── EventBatch(events...) ────────│  (chunked, 256 events/frame)
       │<── SyncComplete ─────────────────│
       │                                  │
       │── EventBatch(my_events...) ─────>│
       │── SyncComplete ─────────────────>│
       │                                  │
       │   (connection closed)            │
```

Events are **idempotent** -- if a session is interrupted mid-transfer, the next sync only sends what's still missing. The Version Vector tracks exactly what each peer has already seen.

---

## Installation

### Download pre-built binary (recommended)

```bash
# Linux x86_64
curl -fsSL -o zamsync \
  https://github.com/Etoile-Bleu/ZamSync/releases/latest/download/zamsync-linux-x86_64
chmod +x zamsync && sudo mv zamsync /usr/local/bin/

# Linux ARM64 (Raspberry Pi 4, AWS Graviton)
curl -fsSL -o zamsync \
  https://github.com/Etoile-Bleu/ZamSync/releases/latest/download/zamsync-linux-aarch64
chmod +x zamsync && sudo mv zamsync /usr/local/bin/

# Linux ARMv7 (Raspberry Pi 2 / 3)
curl -fsSL -o zamsync \
  https://github.com/Etoile-Bleu/ZamSync/releases/latest/download/zamsync-linux-armv7
chmod +x zamsync && sudo mv zamsync /usr/local/bin/

# Windows (PowerShell)
Invoke-WebRequest `
  -Uri "https://github.com/Etoile-Bleu/ZamSync/releases/latest/download/zamsync-windows-x86_64.exe" `
  -OutFile zamsync.exe
```

Verify the download:

```bash
sha256sum -c SHA256SUMS.txt
```

### Docker

```bash
# Pull the multi-arch image (amd64, arm64, armv7 -- same tag)
docker pull ghcr.io/etoile-bleu/zamsync:latest

# Run a node
docker run -d \
  -v /var/lib/zamsync:/var/lib/zamsync \
  -p 7000:7000 \
  --name zamsync \
  ghcr.io/etoile-bleu/zamsync:latest \
  serve /var/lib/zamsync 0.0.0.0:7000
```

### Build from source

```bash
git clone https://github.com/Etoile-Bleu/ZamSync.git
cd ZamSync
cargo build --release
# binary at: target/release/zamsync
```

---

## Quick Start -- Two nodes on localhost

```bash
# Terminal 1: start the server node
zamsync serve ./node-a 0.0.0.0:7000

# Terminal 2: submit an event on node B, then sync to A
zamsync submit ./node-b '{"patient": "P-001", "type": "visit"}'
zamsync sync   ./node-b 127.0.0.1:7000 $(cat ./node-a/.node_id)

# Verify node A received it
zamsync info ./node-a
```

Node identity is stored in `<data-dir>/.node_id` and generated automatically on first start. Set `RUST_LOG=info` to see structured sync traces.

---

## CLI Reference

### `zamsync info <data-dir>`

Show node status: node ID, event count, peer Version Vectors.

```bash
zamsync info /var/lib/clinic-a
```

---

### `zamsync submit <data-dir> <payload> [flags]`

Append an event to the local WAL. The event is durable immediately after the command returns.

```bash
# Plain text
zamsync submit /var/lib/clinic-a "patient check-in"

# JSON (validated client-side)
zamsync submit /var/lib/clinic-a '{"id": "P-042", "type": "visit"}' --schema json

# JSON with required fields
zamsync submit /var/lib/clinic-a '{"id": "P-042", "ward": "3B"}' --schema json+id,ward

# Encrypted WAL
zamsync submit /var/lib/clinic-a "sensitive record" --key-file /var/lib/clinic-a/data.key
```

| Flag | Description |
|---|---|
| `--schema none` | No validation (default) |
| `--schema json` | Must be valid JSON |
| `--schema json+f1,f2` | Valid JSON with required fields `f1` and `f2` |
| `--key-file <path>` | Encrypt with this key (generated by `keygen`) |

---

### `zamsync serve <data-dir> <bind-addr> [flags]`

Accept incoming sync sessions continuously. Run this on the hub / hospital node.

```bash
# Plain TCP
zamsync serve /var/lib/hub 0.0.0.0:7000

# mTLS -- only nodes with a certificate signed by the hub CA can connect
zamsync serve /var/lib/hub 0.0.0.0:7000 --tls

# Access control: each clinic only gets back its own events
zamsync serve /var/lib/hub 0.0.0.0:7000 --tls --policy own

# With Prometheus metrics
zamsync serve /var/lib/hub 0.0.0.0:7000 --tls --policy own --metrics 0.0.0.0:9090
```

| Flag | Description |
|---|---|
| `--tls` | Enable mTLS (reads `<data-dir>/tls/`) |
| `--policy all` | Serve all events to every peer (default) |
| `--policy own` | Each peer only receives events it originally submitted |
| `--schema <mode>` | Reject incoming events that fail validation |
| `--metrics <addr>` | Expose Prometheus `/metrics` endpoint |
| `--key-file <path>` | WAL encryption key |

---

### `zamsync sync <data-dir> <peer-addr> <peer-id> [flags]`

Pull events from a remote peer (initiator role). Typically run on clinic nodes.

```bash
# Pull from hub
zamsync sync /var/lib/clinic-a 192.168.1.10:7000 $(cat /var/lib/hub/.node_id)

# Over mTLS
zamsync sync /var/lib/clinic-a 192.168.1.10:7000 $(cat /var/lib/hub/.node_id) --tls
```

Retries automatically with exponential backoff (5 attempts). Safe to run while offline -- it will try and exit cleanly.

---

### `zamsync daemon <data-dir> <peer-addr> <peer-id> [flags]`

Autonomous periodic sync. Runs forever, syncing every `--interval` seconds. Designed for clinic nodes where a cron job or systemd service runs ZamSync.

```bash
# Sync every 5 minutes over mTLS
zamsync daemon /var/lib/clinic-a 192.168.1.10:7000 $(cat /var/lib/hub/.node_id) \
  --tls --interval 300
```

---

### `zamsync keygen <data-dir>`

Generate a **hub CA** + hub node certificate + WAL encryption key. Run once on the hub node. The CA private key (`ca.key`) must stay on the hub.

```bash
zamsync keygen /var/lib/hub
# Creates:
#   /var/lib/hub/tls/ca.crt    -- CA cert (safe to distribute to clinics)
#   /var/lib/hub/tls/ca.key    -- CA private key (keep secret, hub only)
#   /var/lib/hub/tls/node.crt  -- hub node cert
#   /var/lib/hub/tls/node.key  -- hub node key
#   /var/lib/hub/data.key      -- WAL encryption key
```

---

### `zamsync sign <clinic-dir> --ca <hub-dir>`

Sign a new clinic node certificate with the hub CA. Run on the hub after `keygen`. The clinic gets a certificate that the hub will accept at mTLS handshake.

```bash
zamsync sign /var/lib/clinic-a --ca /var/lib/hub
# Creates in /var/lib/clinic-a/tls/:
#   ca.crt    -- same CA cert as the hub (for mTLS trust)
#   node.crt  -- clinic cert signed by hub CA
#   node.key  -- clinic private key
#   data.key  -- per-clinic WAL encryption key
```

After signing, copy `/var/lib/clinic-a/` to the clinic device.

---

### `zamsync rekey <data-dir> --old-key <path> --new-key <path>`

Rotate the WAL encryption key. Reads all records with the old key, atomically re-encrypts with the new key (temp file + rename -- crash-safe).

```bash
# Generate a new key first
zamsync keygen /tmp/new-keygen
cp /tmp/new-keygen/data.key /var/lib/clinic-a/data.key.new

# Rotate
zamsync rekey /var/lib/clinic-a \
  --old-key /var/lib/clinic-a/data.key \
  --new-key /var/lib/clinic-a/data.key.new

# Replace key file
mv /var/lib/clinic-a/data.key.new /var/lib/clinic-a/data.key
```

---

### `zamsync audit <data-dir> [flags]`

Inspect the immutable audit trail. Every event includes timestamp, origin node, sequence number, payload size, and SHA-256 integrity hash.

```bash
# Human-readable output
zamsync audit /var/lib/clinic-a

# JSON Lines (pipe to jq, grep, etc.)
zamsync audit /var/lib/clinic-a --format json | jq .

# Filter by time range and node
zamsync audit /var/lib/clinic-a \
  --since 1700000000000 \
  --node 42 \
  --format json

# Encrypted WAL
zamsync audit /var/lib/clinic-a --key-file /var/lib/clinic-a/data.key
```

---

### `zamsync compact <data-dir>`

Remove WAL records that have been confirmed as received by all known peers. Reclaims disk space without affecting sync correctness.

```bash
zamsync compact /var/lib/hub
```

---

### `zamsync bench <data-dir> [--events N]`

Benchmark WAL throughput on the target hardware.

```bash
zamsync bench /tmp/bench-data --events 100000
# ~321k events/sec on x86_64, ~125 bytes/WAL record
```

---

## PKI Setup -- Multi-Clinic Deployment

This is the recommended setup for a hub-and-spoke topology (one hospital, multiple clinics).

```
                    ┌──────────────┐
                    │   Hub / CA   │
                    │  (hospital)  │
                    └──────┬───────┘
               signs       │       signs
          ┌────────────────┤────────────────┐
          │                │                │
   ┌──────▼──────┐  ┌──────▼──────┐  ┌─────▼───────┐
   │  Clinic A   │  │  Clinic B   │  │  Clinic C   │
   │  node.crt   │  │  node.crt   │  │  node.crt   │
   └─────────────┘  └─────────────┘  └─────────────┘
          All share the same ca.crt -- mTLS enforced
```

```bash
# 1. Initialize the hub (once)
zamsync keygen /var/lib/hub

# 2. For each clinic, sign a certificate from the hub
zamsync sign /var/lib/clinic-a --ca /var/lib/hub
zamsync sign /var/lib/clinic-b --ca /var/lib/hub

# 3. Copy clinic directories to their respective devices
scp -r /var/lib/clinic-a pi@clinic-a.local:/var/lib/zamsync

# 4. Start the hub server
zamsync serve /var/lib/hub 0.0.0.0:7000 --tls --policy own

# 5. Clinics sync
zamsync daemon /var/lib/zamsync 192.168.1.10:7000 $(cat /var/lib/hub/.node_id) \
  --tls --key-file /var/lib/zamsync/data.key --interval 300
```

A rogue node that presents a certificate from a different CA is **rejected at the TLS handshake** -- no data is exchanged.

---

## WAL Encryption

The WAL is encrypted at rest using **ChaCha20-Poly1305 AEAD**:

- A random 96-bit nonce is generated per record
- Each record is independently authenticated -- tampering is detected on read
- The key is stored in `data.key` (32 random bytes, base64-encoded)

```bash
# All commands that read or write the WAL accept --key-file
zamsync submit  /var/lib/node "event" --key-file /var/lib/node/data.key
zamsync serve   /var/lib/node 0.0.0.0:7000 --key-file /var/lib/node/data.key
zamsync sync    /var/lib/node 192.168.1.10:7000 <peer-id> --key-file /var/lib/node/data.key
zamsync audit   /var/lib/node --key-file /var/lib/node/data.key
zamsync compact /var/lib/node --key-file /var/lib/node/data.key
```

---

## Access Control

`--policy own` on `serve` enforces per-tenant data isolation. Each clinic receives only the events it originally submitted:

```
Clinic A submits events A1, A2, A3
Clinic B submits events B1, B2

When clinic A syncs:  receives A1, A2, A3  (not B1, B2)
When clinic B syncs:  receives B1, B2       (not A1, A2, A3)
The hub itself sees all events.
```

This is enforced server-side by origin `NodeId` -- it cannot be bypassed by the client.

---

## Prometheus Metrics

When `--metrics <addr>` is set, ZamSync exposes a `/metrics` endpoint:

| Metric | Type | Description |
|---|---|---|
| `zamsync_events_submitted_total` | Counter | Events appended to local WAL |
| `zamsync_events_sent_total` | Counter | Events sent to a peer |
| `zamsync_events_received_total` | Counter | Events received from a peer |
| `zamsync_sync_duration_seconds` | Histogram | Full sync session duration |
| `zamsync_vv_drift` | Gauge | Version Vector gap vs peer |

```bash
zamsync serve /var/lib/hub 0.0.0.0:7000 --metrics 0.0.0.0:9090
curl http://localhost:9090/metrics
```

---

## Systemd Service

For unattended clinic nodes, deploy ZamSync as a systemd service:

```ini
# /etc/systemd/system/zamsync.service
[Unit]
Description=ZamSync Clinic Sync Daemon
After=network.target

[Service]
Type=simple
User=zamsync
ExecStart=/usr/local/bin/zamsync daemon /var/lib/zamsync \
    192.168.1.10:7000 YOUR_HUB_NODE_ID \
    --tls \
    --key-file /var/lib/zamsync/data.key \
    --interval 300 \
    --metrics 0.0.0.0:9090
Restart=always
RestartSec=10
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
```

```bash
sudo systemctl enable --now zamsync
journalctl -u zamsync -f
```

---

## Docker Deployment

```yaml
# docker-compose.yml for a hub node
services:
  zamsync-hub:
    image: ghcr.io/etoile-bleu/zamsync:latest
    restart: unless-stopped
    ports:
      - "7000:7000"
      - "9090:9090"
    volumes:
      - /var/lib/zamsync:/var/lib/zamsync
    command: >
      serve /var/lib/zamsync 0.0.0.0:7000
      --tls
      --policy own
      --key-file /var/lib/zamsync/data.key
      --metrics 0.0.0.0:9090
```

The image is published to GHCR and works natively on `linux/amd64`, `linux/arm64`, and `linux/arm/v7` -- no emulation, same tag.

---

## Embedding in a Rust Application

ZamSync is a library. Add it to `Cargo.toml`:

```toml
[dependencies]
zamsync-storage = { git = "https://github.com/Etoile-Bleu/ZamSync.git" }
zamsync-core    = { git = "https://github.com/Etoile-Bleu/ZamSync.git" }
```

Implement `StateStore` to project events into your domain model:

```rust
use zamsync_core::{ports::StateStore, Event, SequenceNumber, ZamResult};
use zamsync_storage::ZamEngine;

struct PatientIndex {
    records: std::collections::HashMap<String, serde_json::Value>,
}

impl StateStore for PatientIndex {
    fn apply_event(&mut self, _seq: SequenceNumber, event: &Event) -> ZamResult<()> {
        if let Ok(record) = serde_json::from_slice(&event.payload) {
            let id: String = record["id"].as_str().unwrap_or("unknown").to_string();
            self.records.insert(id, record);
        }
        Ok(())
    }
    fn last_applied_seq(&self) -> Option<SequenceNumber> { None }
}

fn main() -> zamsync_core::ZamResult<()> {
    let index = PatientIndex { records: Default::default() };
    let mut engine = ZamEngine::open_wal("./data", zamsync_core::NodeId(1), index)?;

    engine.submit(1, br#"{"id": "P-001", "name": "Dorji"}"#.to_vec())?;

    Ok(())
}
```

The engine is generic over `EventStore`, `PeerStore`, and `StateStore` -- swap any component for testing without touching your domain logic.

---

## Network Resilience Test

ZamSync ships with a Toxiproxy-based E2E test that simulates real Bhutan 2G conditions:

- 600ms latency + 100ms jitter
- 30 KB/s bandwidth cap
- Mid-sync connection cut (connection killed after 2 seconds)
- Verify: 5,000 events transferred with zero loss or duplication after reconnect

```bash
docker compose -f tests/docker-compose.test.yml up --build --abort-on-container-exit
```

See [tests/README.md](tests/README.md) for details.

---

## Building

```bash
# Native
cargo build --release

# Cross-compile for ARM (requires `cross`)
cross build --release --target aarch64-unknown-linux-gnu
cross build --release --target armv7-unknown-linux-gnueabihf

# Tests
cargo test --workspace

# Format + lint
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
```

---

## Releasing

Releases are fully automated. No manual `git tag` needed.

1. Go to **Actions → Release → Run workflow**
2. Enter the version number (e.g. `1.2.0`)
3. Click **Run workflow**

The workflow bumps `Cargo.toml`, commits to `main`, creates the tag, then triggers the build matrix that produces 4 platform binaries, a GitHub Release with checksums, and a Docker multi-arch image on GHCR.

---

## License

MIT -- see [LICENSE](LICENSE).

---

## Roadmap

See [ROADMAP.md](ROADMAP.md) for completed phases and planned work.

Key items in progress:

- `zamsync project <data-dir> --target postgres://...` -- WAL projection service to SQL/NoSQL databases
- `zamsync stream <data-dir>` -- SSE / WebSocket event stream for real-time consumers
- Python and Node.js SDKs
- REST API embedded in `serve`
