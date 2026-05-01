# Data Sharing

`cap watch` uploads GPU availability observations to Capacitor's ingestion API.
This is part of the product: Capacitor builds public GPU market intelligence
from observed availability, pricing, and provider metadata.

Capacitor does not upload your Vast.ai or Lambda Cloud API keys. Provider
credentials are stored locally in the operating system keychain.

Private beta registration uses `cap init --beta-token <token>`. The beta token
and backend-minted ingest token are also stored locally in the operating system
keychain so registration and sync can retry after temporary outages.

When the ingestion API or network is unavailable, Capacitor temporarily caches
observations locally and retries sync later. Watching and terminal alerts keep
working during those outages.

Uploaded observations may include normalized fields such as provider, offer id,
GPU name, GPU count, GPU RAM, price, reliability, verification status, region,
and a hashed host identifier. Raw provider offer payloads may be included so the
project can improve normalization over time.
