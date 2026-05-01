use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use cap_cache::ObservationCache;
use cap_core::{IngestBatch, InstallRegistration, OfferObservation, WatchSpec, score_deal};
use cap_ingest::IngestClient;
use cap_providers::{Provider, ProviderConfig, ProviderRegistry, available_providers};
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
const DEFAULT_PROVIDER: &str = "vast";

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
    #[arg(long, conflicts_with = "providers")]
    provider: Option<String>,
    /// Comma-separated providers to watch, for example: vast,lambda.
    #[arg(long, value_name = "PROVIDERS", conflicts_with = "provider")]
    providers: Option<String>,
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

struct ProviderWatch {
    spec: WatchSpec,
    provider: Box<dyn Provider>,
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

    let provider_names =
        resolve_provider_names(args.provider.as_deref(), args.providers.as_deref())?;
    let specs = watch_specs_for(&args, &provider_names)?;
    let registry = ProviderRegistry::new(provider_config_for(&provider_names)?);
    let watches = specs
        .into_iter()
        .map(|spec| {
            let provider = registry.build(&spec.provider)?;
            Ok(ProviderWatch { spec, provider })
        })
        .collect::<Result<Vec<_>, cap_providers::ProviderError>>()?;
    let cache = ObservationCache::connect(&paths.cache_path).await?;
    let ingest = IngestClient::fixed()?;

    loop {
        if !config.ingestion_registered || load_secret(INGEST_TOKEN_USER).is_err() {
            let beta_token = load_secret(BETA_TOKEN_USER).ok();
            try_register_installation(&paths, &mut config, beta_token.as_deref()).await;
        }

        if let Err(error) = run_watch_cycle(
            &watches,
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

        time::sleep(Duration::from_secs(args.poll_interval)).await;
    }

    Ok(())
}

async fn run_watch_cycle(
    watches: &[ProviderWatch],
    cache: &ObservationCache,
    ingest: &IngestClient,
    installation_id: Uuid,
) -> Result<()> {
    let observations = collect_observations(watches).await?;

    if observations.is_empty() {
        println!(
            "No matching offers found for providers: {}.",
            watches
                .iter()
                .map(|watch| watch.spec.provider.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    } else {
        print_observations(watches, &observations);
    }

    cache.insert_observations(&observations).await?;
    sync_cached_observations(cache, ingest, installation_id).await;
    Ok(())
}

async fn collect_observations(watches: &[ProviderWatch]) -> Result<Vec<OfferObservation>> {
    let mut observations = Vec::new();
    let mut failed_providers = Vec::new();

    for watch in watches {
        match watch.provider.search(&watch.spec).await {
            Ok(provider_observations) => observations.extend(provider_observations),
            Err(error) => {
                println!(
                    "Provider `{}` failed but other providers will continue: {error:#}",
                    watch.spec.provider
                );
                failed_providers.push(watch.spec.provider.clone());
            }
        }
    }

    if failed_providers.len() == watches.len() {
        return Err(anyhow!(
            "all providers failed: {}",
            failed_providers.join(", ")
        ));
    }

    Ok(observations)
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

fn print_observations(watches: &[ProviderWatch], observations: &[OfferObservation]) {
    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    let multi_provider = watches.len() > 1;
    if multi_provider {
        table.set_header(vec![
            "Provider",
            "GPU",
            "GPUs",
            "$/hr",
            "Reliability",
            "Verified",
            "Region",
            "Deal",
        ]);
    } else {
        table.set_header(vec![
            "GPU",
            "GPUs",
            "$/hr",
            "Reliability",
            "Verified",
            "Region",
            "Deal",
        ]);
    }

    for observation in observations {
        let spec = watches
            .iter()
            .find(|watch| {
                watch
                    .spec
                    .provider
                    .eq_ignore_ascii_case(&observation.provider)
            })
            .map(|watch| &watch.spec)
            .unwrap_or(&watches[0].spec);
        let deal = score_deal(spec, observation);
        let deal_label = if deal.deal_score >= 50.0 {
            "interesting"
        } else {
            "match"
        };

        let mut row = Vec::new();
        if multi_provider {
            row.push(Cell::new(&observation.provider));
        }

        row.extend(vec![
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
        table.add_row(row);
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

fn resolve_provider_names(provider: Option<&str>, providers: Option<&str>) -> Result<Vec<String>> {
    if let Some(provider_list) = providers {
        return parse_provider_list(provider_list);
    }

    let provider = provider.unwrap_or(DEFAULT_PROVIDER).trim();
    if provider.eq_ignore_ascii_case("all") {
        return Ok(available_providers()
            .iter()
            .map(|provider| provider.to_string())
            .collect());
    }

    parse_provider_list(provider)
}

fn parse_provider_list(provider_list: &str) -> Result<Vec<String>> {
    let mut providers = Vec::new();
    for provider in provider_list.split(',') {
        let provider = provider.trim();
        if provider.is_empty() {
            return Err(anyhow!("provider list contains an empty provider name"));
        }

        let provider = provider.to_ascii_lowercase();
        if !available_providers().contains(&provider.as_str()) {
            return Err(anyhow!(
                "unknown provider `{provider}`; supported providers: {}",
                provider_help()
            ));
        }

        if !providers.contains(&provider) {
            providers.push(provider);
        }
    }

    if providers.is_empty() {
        return Err(anyhow!("at least one provider is required"));
    }

    Ok(providers)
}

fn watch_specs_for(args: &WatchArgs, provider_names: &[String]) -> Result<Vec<WatchSpec>> {
    provider_names
        .iter()
        .map(|provider| {
            let spec = WatchSpec {
                provider: provider.clone(),
                gpu_filters: args.gpu_filters.clone(),
                max_price: args.max_price,
                verified: args.verified,
                min_reliability: args.min_reliability,
                min_gpus: args.min_gpus,
                poll_interval_secs: args.poll_interval,
            };
            spec.validate()?;
            Ok(spec)
        })
        .collect()
}

fn provider_config_for(providers: &[String]) -> Result<ProviderConfig> {
    provider_config_for_with_loaders(
        providers,
        || {
            let vast_api_key = load_provider_secret(
                VAST_API_KEY_USER,
                &[LEGACY_PROVIDER_VAST_USER, LEGACY_VAST_API_KEY_USER],
            )
            .context(
                "missing Vast.ai API key; run `cap config set provider.vast.api-key <token>`",
            )?;
            Ok(vast_api_key)
        },
        load_secret,
    )
}

fn provider_config_for_with_loaders<V, L>(
    providers: &[String],
    load_vast_api_key: V,
    load_secret_by_user: L,
) -> Result<ProviderConfig>
where
    V: Fn() -> Result<String>,
    L: Fn(&str) -> Result<String>,
{
    let mut config = ProviderConfig::default();

    for provider in providers {
        match provider.as_str() {
            "vast" => {
                config.vast_api_key = Some(load_vast_api_key()?);
            }
            "lambda" => {
                config.lambda_api_key = Some(load_secret_by_user(LAMBDA_API_KEY_USER).context(
                    "missing Lambda Cloud API key; run `cap config set provider.lambda.api-key <token>`",
                )?);
            }
            _ => {
                return Err(anyhow!(
                    "unknown provider `{provider}`; supported providers: {}",
                    provider_help()
                ));
            }
        }
    }

    Ok(config)
}

#[allow(dead_code)]
fn provider_help() -> String {
    available_providers().join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockProvider {
        name: &'static str,
        observations: Vec<OfferObservation>,
        should_fail: bool,
    }

    #[async_trait::async_trait]
    impl Provider for MockProvider {
        fn name(&self) -> &'static str {
            self.name
        }

        async fn search(
            &self,
            _spec: &WatchSpec,
        ) -> std::result::Result<Vec<OfferObservation>, cap_providers::ProviderError> {
            if self.should_fail {
                return Err(cap_providers::ProviderError::InvalidResponse(
                    "mock failure".to_string(),
                ));
            }

            Ok(self.observations.clone())
        }
    }

    #[test]
    fn provider_selection_defaults_to_vast() {
        assert_eq!(
            resolve_provider_names(None, None).unwrap(),
            vec!["vast".to_string()]
        );
    }

    #[test]
    fn provider_selection_supports_all() {
        assert_eq!(
            resolve_provider_names(Some("all"), None).unwrap(),
            available_providers()
                .iter()
                .map(|provider| provider.to_string())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn provider_selection_parses_deduped_lists() {
        assert_eq!(
            resolve_provider_names(None, Some(" vast,lambda,VAST ")).unwrap(),
            vec!["vast".to_string(), "lambda".to_string()]
        );
    }

    #[test]
    fn provider_selection_rejects_unknown_provider() {
        let error = resolve_provider_names(None, Some("vast,runpod")).unwrap_err();
        assert!(error.to_string().contains("unknown provider `runpod`"));
    }

    #[test]
    fn clap_rejects_provider_and_providers_together() {
        let result = Cli::try_parse_from([
            "cap",
            "watch",
            "--provider",
            "vast",
            "--providers",
            "vast,lambda",
            "--gpu",
            "H100",
        ]);

        assert!(result.is_err());
    }

    #[test]
    fn provider_config_loads_only_selected_provider_secret() {
        let config = provider_config_for_with_loaders(
            &["lambda".to_string()],
            || panic!("vast credential should not be loaded"),
            |user| {
                assert_eq!(user, LAMBDA_API_KEY_USER);
                Ok("lambda-token".to_string())
            },
        )
        .unwrap();

        assert_eq!(config.lambda_api_key.as_deref(), Some("lambda-token"));
        assert!(config.vast_api_key.is_none());
    }

    #[test]
    fn provider_config_loads_all_selected_provider_secrets() {
        let config = provider_config_for_with_loaders(
            &["vast".to_string(), "lambda".to_string()],
            || Ok("vast-token".to_string()),
            |_| Ok("lambda-token".to_string()),
        )
        .unwrap();

        assert_eq!(config.vast_api_key.as_deref(), Some("vast-token"));
        assert_eq!(config.lambda_api_key.as_deref(), Some("lambda-token"));
    }

    #[tokio::test]
    async fn collect_observations_merges_provider_results() {
        let watches = vec![
            provider_watch(
                "vast",
                vec![test_observation("vast", "vast-offer", "H100 SXM", 1, 2.5)],
                false,
            ),
            provider_watch(
                "lambda",
                vec![test_observation(
                    "lambda",
                    "gpu_1x_h100:us-east-1",
                    "H100",
                    1,
                    4.29,
                )],
                false,
            ),
        ];

        let observations = collect_observations(&watches).await.unwrap();

        assert_eq!(observations.len(), 2);
        assert!(observations.iter().any(|item| item.provider == "vast"));
        assert!(observations.iter().any(|item| item.provider == "lambda"));
    }

    #[tokio::test]
    async fn collect_observations_keeps_partial_results() {
        let watches = vec![
            provider_watch("vast", Vec::new(), true),
            provider_watch(
                "lambda",
                vec![test_observation(
                    "lambda",
                    "gpu_1x_h100:us-east-1",
                    "H100",
                    1,
                    4.29,
                )],
                false,
            ),
        ];

        let observations = collect_observations(&watches).await.unwrap();

        assert_eq!(observations.len(), 1);
        assert_eq!(observations[0].provider, "lambda");
    }

    #[tokio::test]
    async fn collect_observations_errors_when_all_providers_fail() {
        let watches = vec![
            provider_watch("vast", Vec::new(), true),
            provider_watch("lambda", Vec::new(), true),
        ];

        let error = collect_observations(&watches).await.unwrap_err();

        assert!(error.to_string().contains("all providers failed"));
    }

    fn provider_watch(
        provider: &'static str,
        observations: Vec<OfferObservation>,
        should_fail: bool,
    ) -> ProviderWatch {
        ProviderWatch {
            spec: WatchSpec {
                provider: provider.to_string(),
                gpu_filters: vec!["H100".to_string()],
                max_price: Some(10.0),
                verified: false,
                min_reliability: None,
                min_gpus: None,
                poll_interval_secs: 60,
            },
            provider: Box::new(MockProvider {
                name: provider,
                observations,
                should_fail,
            }),
        }
    }

    fn test_observation(
        provider: &str,
        provider_offer_id: &str,
        gpu_name: &str,
        num_gpus: u32,
        price_usd_per_hour: f64,
    ) -> OfferObservation {
        OfferObservation {
            observation_id: Uuid::new_v4(),
            observed_at: chrono::Utc::now(),
            provider: provider.to_string(),
            provider_offer_id: provider_offer_id.to_string(),
            gpu_name: gpu_name.to_string(),
            num_gpus,
            gpu_ram_gb: Some(80.0),
            price_usd_per_hour,
            reliability_score: Some(1.0),
            verified: true,
            rentable: true,
            region: Some("us-east-1".to_string()),
            host_id_hash: None,
            raw_provider_payload: serde_json::json!({}),
        }
    }
}
