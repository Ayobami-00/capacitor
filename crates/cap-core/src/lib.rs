use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("provider is required")]
    MissingProvider,
    #[error("at least one GPU filter is required")]
    MissingGpuFilter,
    #[error("max price must be greater than 0")]
    InvalidMaxPrice,
    #[error("minimum reliability must be between 0 and 1")]
    InvalidReliability,
    #[error("minimum GPU count must be greater than 0")]
    InvalidMinGpus,
    #[error("poll interval must be at least 10 seconds")]
    InvalidPollInterval,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct WatchSpec {
    pub provider: String,
    pub gpu_filters: Vec<String>,
    pub max_price: Option<f64>,
    pub verified: bool,
    pub min_reliability: Option<f64>,
    pub min_gpus: Option<u32>,
    pub poll_interval_secs: u64,
}

impl WatchSpec {
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.provider.trim().is_empty() {
            return Err(CoreError::MissingProvider);
        }

        if self.gpu_filters.iter().all(|gpu| gpu.trim().is_empty()) {
            return Err(CoreError::MissingGpuFilter);
        }

        if matches!(self.max_price, Some(price) if price <= 0.0) {
            return Err(CoreError::InvalidMaxPrice);
        }

        if matches!(self.min_reliability, Some(reliability) if !(0.0..=1.0).contains(&reliability))
        {
            return Err(CoreError::InvalidReliability);
        }

        if matches!(self.min_gpus, Some(0)) {
            return Err(CoreError::InvalidMinGpus);
        }

        if self.poll_interval_secs < 10 {
            return Err(CoreError::InvalidPollInterval);
        }

        Ok(())
    }

    pub fn matches_observation(&self, observation: &OfferObservation) -> bool {
        if !self.provider.eq_ignore_ascii_case(&observation.provider) {
            return false;
        }

        if self.verified && !observation.verified {
            return false;
        }

        if let Some(max_price) = self.max_price
            && observation.price_usd_per_hour > max_price
        {
            return false;
        }

        if let Some(min_reliability) = self.min_reliability
            && observation.reliability_score.unwrap_or(0.0) < min_reliability
        {
            return false;
        }

        if let Some(min_gpus) = self.min_gpus
            && observation.num_gpus < min_gpus
        {
            return false;
        }

        let gpu_name = observation.gpu_name.to_ascii_lowercase();
        self.gpu_filters
            .iter()
            .any(|filter| gpu_name.contains(&filter.to_ascii_lowercase()))
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct OfferObservation {
    pub observation_id: Uuid,
    pub observed_at: DateTime<Utc>,
    pub provider: String,
    pub provider_offer_id: String,
    pub gpu_name: String,
    pub num_gpus: u32,
    pub gpu_ram_gb: Option<f64>,
    pub price_usd_per_hour: f64,
    pub reliability_score: Option<f64>,
    pub verified: bool,
    pub rentable: bool,
    pub region: Option<String>,
    pub host_id_hash: Option<String>,
    pub raw_provider_payload: Value,
}

impl OfferObservation {
    pub fn cache_key(&self) -> String {
        format!(
            "{}:{}:{}",
            self.provider,
            self.provider_offer_id,
            self.observed_at.timestamp()
        )
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct DealCandidate {
    pub observation_id: Uuid,
    pub deal_score: f64,
    pub reason_labels: Vec<String>,
}

pub fn score_deal(spec: &WatchSpec, observation: &OfferObservation) -> DealCandidate {
    let mut score = 0.0;
    let mut reason_labels = Vec::new();

    if let Some(max_price) = spec.max_price {
        let discount = ((max_price - observation.price_usd_per_hour) / max_price).max(0.0);
        if discount >= 0.10 {
            reason_labels.push(format!("{:.0}% below price ceiling", discount * 100.0));
        }
        score += discount * 60.0;
    }

    if observation.verified {
        score += 20.0;
        reason_labels.push("verified host".to_string());
    }

    if let Some(reliability) = observation.reliability_score {
        score += reliability * 20.0;
        if reliability >= 0.98 {
            reason_labels.push("high reliability".to_string());
        }
    }

    DealCandidate {
        observation_id: observation.observation_id,
        deal_score: score,
        reason_labels,
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct InstallRegistration {
    pub installation_id: Uuid,
    pub cli_version: String,
    pub os: String,
    pub arch: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct IngestBatch {
    pub cli_version: String,
    pub installation_id: Uuid,
    pub watch_run_id: Uuid,
    pub observations: Vec<OfferObservation>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct IngestResult {
    pub accepted_count: u64,
    pub duplicate_count: u64,
    pub rejected_count: u64,
    pub retry_after_secs: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_watch_spec_bounds() {
        let spec = WatchSpec {
            provider: "vast".to_string(),
            gpu_filters: vec!["H100".to_string()],
            max_price: Some(3.0),
            verified: true,
            min_reliability: Some(0.98),
            min_gpus: Some(8),
            poll_interval_secs: 60,
        };

        assert!(spec.validate().is_ok());
    }

    #[test]
    fn rejects_tiny_poll_interval() {
        let spec = WatchSpec {
            provider: "vast".to_string(),
            gpu_filters: vec!["H100".to_string()],
            max_price: Some(3.0),
            verified: false,
            min_reliability: None,
            min_gpus: None,
            poll_interval_secs: 1,
        };

        assert!(matches!(
            spec.validate().unwrap_err(),
            CoreError::InvalidPollInterval
        ));
    }

    #[test]
    fn filters_by_minimum_gpu_count() {
        let spec = WatchSpec {
            provider: "vast".to_string(),
            gpu_filters: vec!["H100".to_string()],
            max_price: Some(24.0),
            verified: true,
            min_reliability: Some(0.98),
            min_gpus: Some(8),
            poll_interval_secs: 60,
        };
        let observation = OfferObservation {
            observation_id: Uuid::new_v4(),
            observed_at: Utc::now(),
            provider: "vast".to_string(),
            provider_offer_id: "offer-1".to_string(),
            gpu_name: "H100 SXM".to_string(),
            num_gpus: 4,
            gpu_ram_gb: Some(80.0),
            price_usd_per_hour: 12.0,
            reliability_score: Some(0.99),
            verified: true,
            rentable: true,
            region: Some("US".to_string()),
            host_id_hash: None,
            raw_provider_payload: serde_json::json!({}),
        };

        assert!(!spec.matches_observation(&observation));
    }

    #[test]
    fn score_rewards_verified_reliable_discounted_offers() {
        let spec = WatchSpec {
            provider: "vast".to_string(),
            gpu_filters: vec!["H100".to_string()],
            max_price: Some(4.0),
            verified: true,
            min_reliability: Some(0.98),
            min_gpus: None,
            poll_interval_secs: 60,
        };
        let observation = OfferObservation {
            observation_id: Uuid::new_v4(),
            observed_at: Utc::now(),
            provider: "vast".to_string(),
            provider_offer_id: "offer-1".to_string(),
            gpu_name: "H100 SXM".to_string(),
            num_gpus: 1,
            gpu_ram_gb: Some(80.0),
            price_usd_per_hour: 3.0,
            reliability_score: Some(0.99),
            verified: true,
            rentable: true,
            region: Some("US".to_string()),
            host_id_hash: None,
            raw_provider_payload: serde_json::json!({}),
        };

        let deal = score_deal(&spec, &observation);
        assert!(deal.deal_score > 50.0);
        assert!(deal.reason_labels.contains(&"verified host".to_string()));
    }
}
