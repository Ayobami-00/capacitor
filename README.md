# Capacitor

Capacitor is a Rust CLI agent for finding, reserving, and running workloads on
scarce GPU capacity across cloud GPU providers.

The v0.1 release is a capacity watcher: it monitors Vast.ai offers from your
terminal, filters for the GPUs you care about, alerts when matching capacity
appears, caches observations locally when offline, and contributes availability
data to Capacitor's ingestion backend.

```bash
cap watch \
  --provider vast \
  --gpu H100 \
  --min-gpus 8 \
  --max-price 24.00 \
  --verified \
  --min-reliability 0.98
```

## Status

Capacitor is early and intentionally narrow.

- Current release: `v0.1.0`
- Current provider: Vast.ai
- Current mode: private beta ingestion
- License: Apache-2.0

The first goal is simple: make it easy to watch scarce GPU capacity from the
terminal and collect enough availability history to understand real GPU market
patterns.

## What It Does Today

- Watches Vast.ai offers from the terminal
- Filters by GPU name, GPU count, total price, verification status, and
  reliability
- Prints matching offers in a terminal table
- Highlights interesting deals
- Caches observations locally in SQLite when sync is unavailable
- Uploads GPU availability observations to Capacitor ingestion
- Keeps provider credentials in your operating system keychain

## Install

### Install Prebuilt Binary

```bash
curl -fsSL https://raw.githubusercontent.com/Ayobami-00/capacitor/main/install.sh | sh
```

The installer downloads the latest GitHub Release binary for your platform,
verifies the checksum when possible, and installs `cap` to:

```text
$HOME/.local/bin
```

To install somewhere else:

```bash
curl -fsSL https://raw.githubusercontent.com/Ayobami-00/capacitor/main/install.sh | \
  CAPACITOR_INSTALL_DIR=/usr/local/bin sh
```

Supported prebuilt targets:

```text
aarch64-apple-darwin      # macOS Apple Silicon
x86_64-unknown-linux-gnu  # Linux x64
```

Intel macOS users can still install Capacitor with Cargo or build from source.

v0.1 binaries are unsigned and not notarized. On Linux, OS keychain support
depends on a Secret Service-compatible keyring being available.

### Manual GitHub Release Download

Download the archive for your platform from:

```text
https://github.com/Ayobami-00/capacitor/releases/latest
```

Then unpack it and move `cap` somewhere on your `PATH`.

### Install With Cargo

If you already have Rust installed:

```bash
cargo install --git https://github.com/Ayobami-00/capacitor.git cap-cli --locked
```

### Build From Source

#### Prerequisites

Install Rust stable:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Capacitor uses the Rust 2024 edition, so use a recent stable toolchain.

```bash
git clone https://github.com/Ayobami-00/capacitor.git
cd capacitor
cargo build --workspace
cargo run --bin cap -- --help
```

After any install path, check that the binary is available:

```bash
cap --help
```

## Quickstart

During the private beta, Capacitor v0.1 requires two tokens:

- A Capacitor beta token for ingestion registration
- A Vast.ai API key for reading Vast.ai offers

Initialize Capacitor:

```bash
cap init --beta-token <capacitor-beta-token>
```

Store your Vast.ai API key:

```bash
cap config set provider.vast.api-key <vast-api-key>
```

Check local setup:

```bash
cap doctor
```

Run one watch cycle:

```bash
cap watch \
  --provider vast \
  --gpu H100 \
  --max-price 3.00 \
  --verified \
  --min-reliability 0.98 \
  --once
```

Watch continuously:

```bash
cap watch \
  --provider vast \
  --gpu H100 \
  --max-price 3.00 \
  --verified \
  --min-reliability 0.98 \
  --poll-interval 60
```

Watch for an 8xH100 offer:

```bash
cap watch \
  --provider vast \
  --gpu H100 \
  --min-gpus 8 \
  --max-price 24.00 \
  --verified \
  --min-reliability 0.98
```

`--max-price` is the total offer price per hour, not per-GPU price.

## Commands

### `cap init`

Initializes local config, creates the local SQLite observation cache, registers
the install with Capacitor ingestion, and stores the backend-minted ingest token
in the OS keychain.

```bash
cap init --beta-token <capacitor-beta-token>
```

### `cap config set`

Stores supported provider credentials in the OS keychain.

```bash
cap config set provider.vast.api-key <vast-api-key>
```

### `cap watch`

Watches provider offers, filters matching capacity, prints terminal alerts,
caches observations locally, and syncs observations to Capacitor ingestion.

```bash
cap watch --provider vast --gpu H100
```

Useful filters:

```text
--gpu <name>                 GPU name filter. Can be repeated.
--min-gpus <count>           Minimum number of GPUs in one offer.
--max-price <usd>            Maximum total offer price per hour.
--verified                   Require verified hosts.
--min-reliability <score>    Minimum reliability score between 0 and 1.
--poll-interval <seconds>    Poll interval. Minimum: 10 seconds.
--once                       Run one poll cycle and exit.
```

### `cap doctor`

Prints local readiness checks for config, cache, keychain secrets, ingestion
registration, and cached observation counts.

```bash
cap doctor
```

## Data Sharing

`cap watch` uploads GPU availability observations to Capacitor's fixed ingestion
API. This is part of the product: Capacitor is building public GPU market
intelligence from observed availability, pricing, and provider metadata.

Capacitor does not upload your Vast.ai API key. Provider credentials are stored
locally in your operating system keychain. The Capacitor beta token and
backend-minted ingest token are also stored in the keychain so registration and
sync can retry after temporary outages.

Uploaded observations may include:

- Provider name
- Provider offer id
- GPU name and count
- GPU RAM
- Price per hour
- Reliability score
- Verification and rentable status
- Region
- Hashed host identifier
- Raw provider offer payload

When the ingestion API or network is unavailable, Capacitor keeps watching,
caches observations locally, and retries sync later.

## Example Output

```text
+----------+------+-------+-------------+----------+-------------+-------------+
| GPU      | GPUs | $/hr | Reliability | Verified | Region      | Deal        |
+----------+------+-------+-------------+----------+-------------+-------------+
| H100 NVL | 1    | 1.76 | 0.995       | true     | Florida, US | interesting |
+----------+------+-------+-------------+----------+-------------+-------------+
```

## Architecture

```text
cap watch
  -> provider registry
  -> Vast.ai provider implementation
  -> normalized OfferObservation records
  -> terminal alert
  -> local SQLite cache
  -> Capacitor ingestion API
```

Workspace crates:

```text
crates/
  cap-cli        # binary: cap
  cap-core       # shared domain models, validation, scoring, ingest payloads
  cap-providers  # provider trait, registry, and Vast.ai implementation
  cap-cache      # local SQLite cache for pending observation sync
  cap-ingest     # fixed Capacitor ingestion API client
```

## v0.1.0 Release

The v0.1.0 release establishes the first complete capacity-watching loop:

- Rust workspace and `cap` binary
- Vast.ai provider support
- Terminal watch command
- GPU, GPU-count, price, verification, and reliability filters
- Local SQLite observation cache
- Fixed Capacitor ingestion client
- Keychain storage for provider and ingestion credentials
- Private beta registration via `cap init --beta-token`
- GitHub Release binaries for macOS Apple Silicon, macOS Intel, and Linux x64
- `install.sh` for installing without Rust/Cargo

See [CHANGELOG.md](./CHANGELOG.md) for release notes.

## Roadmap

This roadmap is directional and may change as Capacitor learns from real usage.

### v0.1: Capacity Watcher

- Watch Vast.ai offers from the terminal
- Filter by GPU, GPU count, price, verification status, and reliability
- Alert locally when matching capacity appears
- Cache observations locally when offline
- Upload GPU availability observations to Capacitor ingestion

### v0.3: Provider Expansion

- Add support for more cloud GPU providers
- Normalize provider-specific capacity into one shared observation model
- Compare GPU availability and pricing across providers
- Keep the CLI provider-agnostic as new integrations are added

### v0.4: Reservation Workflow

- Reserve matching GPU capacity from the CLI
- Add confirmation and dry-run flows before spending money
- Track reservation attempts and outcomes
- Make the jump from "capacity found" to "capacity claimed" safer

### v0.5: Workload Runner

- Run containerized workloads on reserved GPUs
- Stream logs and collect workload results
- Add retry policies for failures and interruptions
- Move toward the long-term goal: finding capacity and completing the run

## Development

Run checks:

```bash
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace -- -D warnings
cargo test --workspace
cargo build --release --locked --bin cap
```

Run the CLI locally:

```bash
cargo run --bin cap -- --help
```

Create a release:

```bash
git tag v0.1.0
git push origin main --tags
```

The release workflow builds prebuilt binaries, uploads release assets, and
generates `SHA256SUMS`. Manual release runs should use an existing tag.

## License

Capacitor is open source under the [Apache License 2.0](./LICENSE).
