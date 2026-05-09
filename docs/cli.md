# Capacitor CLI

`cap` watches scarce GPU capacity and contributes availability observations to
Capacitor's fixed ingestion API.

## Install

Install the latest prebuilt release:

```bash
curl -fsSL https://raw.githubusercontent.com/Ayobami-00/capacitor/main/install.sh | sh
```

Install from source:

```bash
cargo install --git https://github.com/Ayobami-00/capacitor.git cap-cli --locked
```

Or run from source:

```bash
cargo run --bin cap -- --help
```

## Commands

```bash
cap init
cap config set provider.vast.api-key <token>
cap config set provider.lambda.api-key <token>
cap config set provider.runpod.api-key <token>
cap watch --provider vast --gpu H100 --min-gpus 8 --max-price 24.00 --verified --min-reliability 0.98
cap watch --provider lambda --gpu H100 --min-gpus 8 --max-price 36.00 --verified --min-reliability 0.98
cap watch --provider runpod --gpu H100 --min-gpus 8 --max-price 36.00 --verified --min-reliability 0.98
cap watch --providers vast,lambda,runpod --gpu H100 --max-price 9.00 --once
cap watch --providers vast,lambda,runpod --gpu H100 --max-price 9.00 --once --format json
cap doctor
```

## `cap watch`

`cap watch` currently supports Vast.ai, Lambda Cloud, and Runpod through the
provider-agnostic command layer. Future providers should be added under
`cap-providers` rather than as new CLI commands.

Supported filters:

```text
--provider <name>             Provider to watch. Use all for every provider.
--providers <names>           Comma-separated providers, for example vast,lambda,runpod.
--gpu <name>                 GPU name filter. Can be repeated.
--min-gpus <count>           Minimum number of GPUs in one offer.
--max-price <usd>            Maximum total offer price per hour.
--verified                   Require verified hosts.
--min-reliability <score>    Minimum reliability score between 0 and 1.
--poll-interval <seconds>    Poll interval. Minimum: 10 seconds.
--once                       Run one poll cycle and exit.
--format <table|json>        Output format. Defaults to table.
```

Examples:

```bash
cap watch --provider vast --gpu H100 --max-price 3.00 --verified --once
cap watch --provider vast --gpu H100 --min-gpus 8 --max-price 24.00 --verified
cap watch --provider lambda --gpu H100 --min-gpus 8 --max-price 36.00 --verified --once
cap watch --provider runpod --gpu H100 --min-gpus 8 --max-price 36.00 --verified --once
cap watch --providers vast,lambda,runpod --gpu H100 --max-price 9.00 --verified --once
cap watch --provider all --gpu H100 --max-price 9.00 --verified --once
cap watch --providers vast,lambda,runpod --gpu H100 --max-price 9.00 --once --format json
```

Lambda Cloud is treated as first-party verified capacity, so `--verified` and
`--min-reliability` can be used with Lambda watches even though Lambda does not
expose marketplace host reliability fields.

Runpod support is Secure Cloud first. Runpod observations are treated as
verified capacity, and `--max-price` is normalized to total hourly price.

Cross-provider watches merge normalized observations into one table and add a
provider column to make results comparable. `--max-price` remains the total
offer or instance price per hour across providers.

`--format json` is intended for automation. JSON output includes provider,
GPU, GPU count, price, reliability, region, observed time, and deal label.
Operational warnings are written to stderr so stdout remains parseable.

## Container Credentials

The OS keychain remains the default secret store. For Docker/headless usage,
`cap` also reads:

```text
CAP_PROVIDER_VAST_API_KEY
CAP_PROVIDER_LAMBDA_API_KEY
CAP_PROVIDER_RUNPOD_API_KEY
CAPACITOR_INGEST_TOKEN
CAPACITOR_SECRET_DIR
```

When the keychain is unavailable, `cap config set` falls back to local files in
`CAPACITOR_SECRET_DIR` or the platform data directory.

## Not Included In The MVP

The MVP intentionally does not expose queue management, contribution toggles,
ingestion endpoint configuration, reporting commands, auto-renting, workload
execution, or Twitter automation.
