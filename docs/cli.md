# Capacitor CLI

`cap` watches scarce GPU capacity and contributes availability observations to
Capacitor's fixed ingestion API.

## Install

```bash
cargo install --git https://github.com/Ayobami-00/capacitor.git cap-cli --locked
```

Or run from source:

```bash
cargo run --bin cap -- --help
```

## Commands

```bash
cap init --beta-token <token>
cap config set provider.vast.api-key <token>
cap config set provider.lambda.api-key <token>
cap watch --provider vast --gpu H100 --min-gpus 8 --max-price 24.00 --verified --min-reliability 0.98
cap watch --provider lambda --gpu H100 --min-gpus 8 --max-price 36.00 --verified --min-reliability 0.98
cap doctor
```

## `cap watch`

`cap watch` currently supports Vast.ai and Lambda Cloud through the
provider-agnostic command layer. Future providers should be added under
`cap-providers` rather than as new CLI commands.

Supported filters:

```text
--gpu <name>                 GPU name filter. Can be repeated.
--min-gpus <count>           Minimum number of GPUs in one offer.
--max-price <usd>            Maximum total offer price per hour.
--verified                   Require verified hosts.
--min-reliability <score>    Minimum reliability score between 0 and 1.
--poll-interval <seconds>    Poll interval. Minimum: 10 seconds.
--once                       Run one poll cycle and exit.
```

Examples:

```bash
cap watch --provider vast --gpu H100 --max-price 3.00 --verified --once
cap watch --provider vast --gpu H100 --min-gpus 8 --max-price 24.00 --verified
cap watch --provider lambda --gpu H100 --min-gpus 8 --max-price 36.00 --verified --once
```

Lambda Cloud is treated as first-party verified capacity, so `--verified` and
`--min-reliability` can be used with Lambda watches even though Lambda does not
expose marketplace host reliability fields.

## Not Included In The MVP

The MVP intentionally does not expose queue management, contribution toggles,
ingestion endpoint configuration, reporting commands, auto-renting, workload
execution, or Twitter automation.
