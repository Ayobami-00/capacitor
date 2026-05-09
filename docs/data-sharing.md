# Data Sharing

`cap watch` uploads GPU availability observations to Capacitor's ingestion API.
This is part of the product: Capacitor builds public GPU market intelligence
from observed availability, pricing, and provider metadata.

Single-provider and cross-provider watch commands use the same data-sharing
path. Cross-provider watches merge normalized observations locally before they
are cached and synced.

Capacitor does not upload your Vast.ai, Lambda Cloud, or Runpod API keys. Provider
credentials are stored locally in the operating system keychain by default.
Docker and headless environments can provide credentials through environment
variables or local secret files instead.

Public registration uses `cap init`. The Capacitor backend mints an ingest token
for the local installation, and only that backend-minted ingest token is stored
locally in the operating system keychain or local fallback secret store so
registration and sync can retry after temporary outages.

When the ingestion API or network is unavailable, Capacitor temporarily caches
observations locally and retries sync later. Watching and terminal alerts keep
working during those outages.

Uploaded observations may include normalized fields such as provider, offer id,
GPU name, GPU count, GPU RAM, price, reliability, verification status, region,
and a hashed host identifier. Raw provider offer payloads may be included so the
project can improve normalization over time.
