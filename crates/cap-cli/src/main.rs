use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use cap_cache::ObservationCache;
use cap_core::{IngestBatch, InstallRegistration, WatchSpec, score_deal};
use cap_ingest::IngestClient;
use cap_providers::{ProviderConfig, ProviderRegistry, available_providers};
use clap::{Args, Parser, Subcommand};
use comfy_table::{Cell, Color, Table, presets::UTF8_FULL};
use directories::ProjectDirs;
use keyring::Entry;
use serde::{Deserialize, Serialize};
use tokio::time;
use uuid::Uuid;

const SERVICE_NAME: &str = "capacitor";
const VAST_API_KEY_USER: &str = "provider.vast.api-key";
const LAMBDA_API_KEY_USER: &str = "provider.lambda.api-key";
const LEGACY_PROVIDER_VAST_USER: &str = "provider.vast";
const LEGACY_VAST_API_KEY_USER: &str = "vast.api-key";
const INGEST_TOKEN_USER: &str = "ingest.token";
const BETA_TOKEN_USER: &str = "beta.token";

#[derive(Parser, Debug)]
#[command(name = "cap")]
#[command(about = "GPU capacity radar for scarce cloud compute", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Initialize local Capacitor config, cache, and ingestion registration.
    Init(InitArgs),
    /// Manage local configuration.
    Config(ConfigCommand),
    /// Watch provider offers and upload availability observations.
    Watch(WatchArgs),
    /// Check local Capacitor readiness.
    Doctor,
}

#[derive(Args, Debug)]
struct InitArgs {
    /// Private beta token used to register this install with Capacitor ingestion.
    #[arg(long)]
    beta_token: Option<String>,
}

#[derive(Args, Debug)]
struct ConfigCommand {
    #[command(subcommand)]
    command: ConfigSubcommand,
}

#[derive(Subcommand, Debug)]
enum ConfigSubcommand {
    /// Store a supported config value.
    Set {
        /// Supported keys: provider.vast.api-key, provider.lambda.api-key
        key: String,
        /// Value to store.
        value: String,
    },
}

#[derive(Args, Debug)]
struct WatchArgs {
    /// Provider to watch.
    #[arg(long, default_value = "vast")]
    provider: String,
    /// GPU name filter. Can be repeated.
    #[arg(long = "gpu", required = true)]
    gpu_filters: Vec<String>,
    /// Maximum price per hour in USD.
    #[arg(long)]
    max_price: Option<f64>,
    /// Require verified hosts.
    #[arg(long)]
    verified: bool,
    /// Minimum reliability score between 0 and 1.
    #[arg(long)]
    min_reliability: Option<f64>,
    /// Minimum number of GPUs required in one offer.
    #[arg(long)]
    min_gpus: Option<u32>,
    /// Poll interval in seconds.
    #[arg(long, default_value_t = 60)]
    poll_interval: u64,
    /// Run one poll cycle and exit.
    #[arg(long)]
    once: bool,
}

#[derive(Clone, Debug)]
struct Paths {
    config_path: PathBuf,
    cache_path: PathBuf,
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct AppConfig {
    installation_id: Option<Uuid>,
    ingestion_registered: bool,
}

impl Paths {
    fn load() -> Result<Self> {
        let dirs = ProjectDirs::from("dev", "capacitor", "cap")
            .ok_or_else(|| anyhow!("could not resolve local config directories"))?;
        Ok(Self {
            config_path: dirs.config_dir().join("config.toml"),
            cache_path: dirs.data_local_dir().join("observations.sqlite"),
        })
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into()),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Command::Init(args) => init(args).await,
        Command::Config(command) => config(command).await,
        Command::Watch(args) => watch(args).await,
        Command::Doctor => doctor().await,
    }
}

async fn init(args: InitArgs) -> Result<()> {
    let paths = Paths::load()?;
    ensure_parent(&paths.config_path)?;
    ensure_parent(&paths.cache_path)?;

    let mut config = load_config(&paths)?;
    let installation_id = config.installation_id.unwrap_or_else(Uuid::new_v4);
    config.installation_id = Some(installation_id);

    let _cache = ObservationCache::connect(&paths.cache_path)
        .await
        .context("failed to initialize local observation cache")?;

    if let Some(beta_token) = args.beta_token.as_deref() {
        store_secret(BETA_TOKEN_USER, beta_token)?;
    }

    let beta_token = args
        .beta_token
        .or_else(|| load_secret(BETA_TOKEN_USER).ok());
    try_register_installation(&paths, &mut config, beta_token.as_deref()).await;

    save_config(&paths, &config)?;

    println!("Config: {}", paths.config_path.display());
    println!("Cache: {}", paths.cache_path.display());
    Ok(())
}

async fn config(command: ConfigCommand) -> Result<()> {
    match command.command {
        ConfigSubcommand::Set { key, value } if key == VAST_API_KEY_USER => {
            store_secret(VAST_API_KEY_USER, &value)?;
            println!("Stored Vast.ai API key in the OS keychain.");
            Ok(())
        }
        ConfigSubcommand::Set { key, value } if key == LAMBDA_API_KEY_USER => {
            store_secret(LAMBDA_API_KEY_USER, &value)?;
            println!("Stored Lambda Cloud API key in the OS keychain.");
            Ok(())
        }
        ConfigSubcommand::Set { key, .. } => Err(anyhow!(
            "unsupported config key `{key}`; supported keys: provider.vast.api-key, provider.lambda.api-key"
        )),
    }
}

async fn watch(args: WatchArgs) -> Result<()> {
    let paths = Paths::load()?;
    ensure_parent(&paths.config_path)?;
    ensure_parent(&paths.cache_path)?;

    let mut config = load_config(&paths)?;
    if config.installation_id.is_none() {
        println!("No installation id found; running initialization first.");
        let installation_id = Uuid::new_v4();
        config.installation_id = Some(installation_id);
        save_config(&paths, &config)?;
    }

    let spec = WatchSpec {
        provider: args.provider,
        gpu_filters: args.gpu_filters,
        max_price: args.max_price,
        verified: args.verified,
        min_reliability: args.min_reliability,
        min_gpus: args.min_gpus,
        poll_interval_secs: args.poll_interval,
    };
    spec.validate()?;

    let registry = ProviderRegistry::new(provider_config_for(&spec.provider)?);
    let provider = registry.build(&spec.provider)?;
    let cache = ObservationCache::connect(&paths.cache_path).await?;
    let ingest = IngestClient::fixed()?;

    loop {
        if !config.ingestion_registered || load_secret(INGEST_TOKEN_USER).is_err() {
            let beta_token = load_secret(BETA_TOKEN_USER).ok();
            try_register_installation(&paths, &mut config, beta_token.as_deref()).await;
        }

        if let Err(error) = run_watch_cycle(
            &spec,
            provider.as_ref(),
            &cache,
            &ingest,
            config.installation_id.expect("installation id exists"),
        )
        .await
        {
            println!("watch cycle failed but will retry: {error:#}");
        }

        if args.once {
            break;
        }

        time::sleep(Duration::from_secs(spec.poll_interval_secs)).await;
    }

    Ok(())
}

async fn run_watch_cycle(
    spec: &WatchSpec,
    provider: &dyn cap_providers::Provider,
    cache: &ObservationCache,
    ingest: &IngestClient,
    installation_id: Uuid,
) -> Result<()> {
    let observations = provider.search(spec).await?;

    if observations.is_empty() {
        println!("No matching {} offers found.", spec.provider);
    } else {
        print_observations(spec, &observations);
    }

    cache.insert_observations(&observations).await?;
    sync_cached_observations(cache, ingest, installation_id).await;
    Ok(())
}

async fn sync_cached_observations(
    cache: &ObservationCache,
    ingest: &IngestClient,
    installation_id: Uuid,
) {
    let token = load_secret(INGEST_TOKEN_USER).ok();
    let pending = match cache.pending_observations(500).await {
        Ok(pending) if pending.is_empty() => return,
        Ok(pending) => pending,
        Err(error) => {
            println!("could not read cached observations for sync: {error}");
            return;
        }
    };

    let batch = IngestBatch {
        cli_version: env!("CARGO_PKG_VERSION").to_string(),
        installation_id,
        watch_run_id: Uuid::new_v4(),
        observations: pending.clone(),
    };

    match ingest.upload_observations(token.as_deref(), &batch).await {
        Ok(result) => {
            if let Err(error) = cache.mark_synced(&pending, chrono::Utc::now()).await {
                println!("observations uploaded but cache sync marker failed: {error}");
                return;
            }
            println!(
                "Synced observations: accepted={}, duplicates={}, rejected={}",
                result.accepted_count, result.duplicate_count, result.rejected_count
            );
        }
        Err(error) => {
            println!("Observation sync is pending; cached data will retry later: {error}");
        }
    }
}

async fn doctor() -> Result<()> {
    let paths = Paths::load()?;
    let config = load_config(&paths).unwrap_or_default();
    let cache_stats = if paths.cache_path.exists() {
        Some(
            ObservationCache::connect(&paths.cache_path)
                .await?
                .stats()
                .await?,
        )
    } else {
        None
    };

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec!["Check", "Status"]);
    table.add_row(vec![
        Cell::new("Config path"),
        Cell::new(paths.config_path.display().to_string()),
    ]);
    table.add_row(vec![
        Cell::new("Cache path"),
        Cell::new(paths.cache_path.display().to_string()),
    ]);
    table.add_row(vec![
        Cell::new("Installation id"),
        Cell::new(
            config
                .installation_id
                .map(|id| id.to_string())
                .unwrap_or_else(|| "missing; run cap init".to_string()),
        ),
    ]);
    table.add_row(vec![
        Cell::new("Vast.ai API key"),
        status_cell(
            load_provider_secret(
                VAST_API_KEY_USER,
                &[LEGACY_PROVIDER_VAST_USER, LEGACY_VAST_API_KEY_USER],
            )
            .is_ok(),
        ),
    ]);
    table.add_row(vec![
        Cell::new("Lambda Cloud API key"),
        status_cell(load_secret(LAMBDA_API_KEY_USER).is_ok()),
    ]);
    table.add_row(vec![
        Cell::new("Ingestion token"),
        status_cell(load_secret(INGEST_TOKEN_USER).is_ok()),
    ]);
    table.add_row(vec![
        Cell::new("Beta token"),
        status_cell(load_secret(BETA_TOKEN_USER).is_ok()),
    ]);
    table.add_row(vec![
        Cell::new("Ingestion registered"),
        status_cell(config.ingestion_registered),
    ]);

    if let Some(stats) = cache_stats {
        table.add_row(vec![
            Cell::new("Cached observations"),
            Cell::new(format!(
                "{} total, {} pending, {} synced",
                stats.total, stats.pending, stats.synced
            )),
        ]);
    } else {
        table.add_row(vec![
            Cell::new("Cached observations"),
            Cell::new("missing; run cap init"),
        ]);
    }

    println!("{table}");
    Ok(())
}

fn print_observations(spec: &WatchSpec, observations: &[cap_core::OfferObservation]) {
    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec![
        "GPU",
        "GPUs",
        "$/hr",
        "Reliability",
        "Verified",
        "Region",
        "Deal",
    ]);

    for observation in observations {
        let deal = score_deal(spec, observation);
        let deal_label = if deal.deal_score >= 50.0 {
            "\u{0007}interesting"
        } else {
            "match"
        };

        table.add_row(vec![
            Cell::new(&observation.gpu_name),
            Cell::new(observation.num_gpus),
            Cell::new(format!("{:.2}", observation.price_usd_per_hour)),
            Cell::new(
                observation
                    .reliability_score
                    .map(|score| format!("{score:.3}"))
                    .unwrap_or_else(|| "unknown".to_string()),
            ),
            Cell::new(observation.verified),
            Cell::new(observation.region.as_deref().unwrap_or("unknown")),
            Cell::new(deal_label),
        ]);
    }

    println!("{table}");
}

fn status_cell(ok: bool) -> Cell {
    if ok {
        Cell::new("ok").fg(Color::Green)
    } else {
        Cell::new("missing").fg(Color::Red)
    }
}

async fn register_installation(installation_id: Uuid, beta_token: Option<&str>) -> Result<String> {
    let ingest = IngestClient::fixed()?;
    let registration = InstallRegistration {
        installation_id,
        cli_version: env!("CARGO_PKG_VERSION").to_string(),
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
    };
    Ok(ingest
        .register(&registration, beta_token)
        .await?
        .ingest_token)
}

async fn try_register_installation(
    paths: &Paths,
    config: &mut AppConfig,
    beta_token: Option<&str>,
) {
    let Some(installation_id) = config.installation_id else {
        println!("Ingestion registration is pending: missing local installation id.");
        return;
    };

    match register_installation(installation_id, beta_token).await {
        Ok(token) => match store_secret(INGEST_TOKEN_USER, &token) {
            Ok(()) => {
                config.ingestion_registered = true;
                if let Err(error) = save_config(paths, config) {
                    println!("Registered ingestion, but failed to update config: {error:#}");
                } else {
                    println!("Registered this install with Capacitor ingestion.");
                }
            }
            Err(error) => {
                config.ingestion_registered = false;
                println!("Ingestion registration is pending: could not store token: {error:#}");
            }
        },
        Err(error) => {
            config.ingestion_registered = false;
            if let Err(save_error) = save_config(paths, config) {
                println!("Could not persist pending ingestion status: {save_error:#}");
            }
            println!("Ingestion registration is pending and will retry later: {error:#}");
        }
    }
}

fn load_config(paths: &Paths) -> Result<AppConfig> {
    if !paths.config_path.exists() {
        return Ok(AppConfig::default());
    }

    let contents = fs::read_to_string(&paths.config_path)
        .with_context(|| format!("failed to read {}", paths.config_path.display()))?;
    toml::from_str(&contents)
        .with_context(|| format!("failed to parse {}", paths.config_path.display()))
}

fn save_config(paths: &Paths, config: &AppConfig) -> Result<()> {
    ensure_parent(&paths.config_path)?;
    let contents = toml::to_string_pretty(config)?;
    fs::write(&paths.config_path, contents)
        .with_context(|| format!("failed to write {}", paths.config_path.display()))
}

fn ensure_parent(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    Ok(())
}

fn store_secret(user: &str, value: &str) -> Result<()> {
    Entry::new(SERVICE_NAME, user)?
        .set_password(value)
        .with_context(|| format!("failed to store `{user}` in the OS keychain"))
}

fn load_secret(user: &str) -> Result<String> {
    Entry::new(SERVICE_NAME, user)?
        .get_password()
        .with_context(|| format!("failed to load `{user}` from the OS keychain"))
}

fn load_provider_secret(primary_user: &str, legacy_users: &[&str]) -> Result<String> {
    let mut last_error = match load_secret(primary_user) {
        Ok(secret) => return Ok(secret),
        Err(error) => error,
    };

    for legacy_user in legacy_users {
        match load_secret(legacy_user) {
            Ok(secret) => return Ok(secret),
            Err(error) => last_error = error,
        }
    }

    Err(last_error)
}

fn provider_config_for(provider: &str) -> Result<ProviderConfig> {
    match provider.to_ascii_lowercase().as_str() {
        "vast" => {
            let vast_api_key = load_provider_secret(
                VAST_API_KEY_USER,
                &[LEGACY_PROVIDER_VAST_USER, LEGACY_VAST_API_KEY_USER],
            )
            .context(
                "missing Vast.ai API key; run `cap config set provider.vast.api-key <token>`",
            )?;
            Ok(ProviderConfig {
                vast_api_key: Some(vast_api_key),
                ..ProviderConfig::default()
            })
        }
        "lambda" => {
            let lambda_api_key = load_secret(LAMBDA_API_KEY_USER)
                .context("missing Lambda Cloud API key; run `cap config set provider.lambda.api-key <token>`")?;
            Ok(ProviderConfig {
                lambda_api_key: Some(lambda_api_key),
                ..ProviderConfig::default()
            })
        }
        _ => Ok(ProviderConfig::default()),
    }
}

#[allow(dead_code)]
fn provider_help() -> String {
    available_providers().join(", ")
}
