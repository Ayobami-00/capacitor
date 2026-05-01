# Changelog

All notable changes to Capacitor will be documented in this file.

## Unreleased

### Added

- Added Lambda Cloud watch support through `--provider lambda`.
- Added `cap config set provider.lambda.api-key <token>` for OS
  keychain-backed Lambda credential storage.
- Added Lambda instance type normalization into Capacitor's shared
  `OfferObservation` model.

## v0.1.0 - 2026-04-30

Initial public CLI release.

### Added

- Added the `cap` Rust CLI binary.
- Added `cap init --beta-token <token>` for private beta ingestion registration.
- Added `cap config set provider.vast.api-key <token>` for OS keychain-backed
  Vast.ai credential storage.
- Added `cap watch` for watching Vast.ai capacity from the terminal.
- Added filters for GPU name, minimum GPU count, max total offer price,
  verified hosts, and minimum reliability.
- Added `--once` for single-cycle checks and `--poll-interval` for continuous
  watch mode.
- Added terminal table output with an `interesting` deal label.
- Added provider-agnostic core models and provider registry.
- Added Vast.ai offer search, normalization, and fixture coverage.
- Added local SQLite observation cache for retrying sync after outages.
- Added fixed Capacitor ingestion client for registration and observation sync.
- Added `cap doctor` for local setup and cache readiness checks.
- Added GitHub Release binaries for macOS Apple Silicon, macOS Intel, and Linux
  x64.
- Added `install.sh` for installing `cap` without requiring Rust/Cargo.
- Added data-sharing documentation and Apache-2.0 license.

### Notes

- v0.1.0 supports Vast.ai only.
- `cap watch` uploads GPU availability observations to Capacitor ingestion.
- Provider API keys are stored locally in the OS keychain and are not uploaded.
- v0.1.0 macOS binaries are unsigned and not notarized.
- Reservation, workload execution, reports, dashboards, and multi-provider
  support are intentionally out of scope for this release.
