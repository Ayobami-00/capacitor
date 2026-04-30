use cap_core::{OfferObservation, WatchSpec};
use chrono::Utc;
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::provider::ProviderError;
use crate::vast::models::VastOffer;

pub fn normalize_search_response(
    raw: &str,
    spec: &WatchSpec,
) -> Result<Vec<OfferObservation>, ProviderError> {
    let value: Value = serde_json::from_str(raw)
        .map_err(|error| ProviderError::InvalidResponse(error.to_string()))?;
    let offers = value
        .get("offers")
        .ok_or_else(|| ProviderError::InvalidResponse("missing offers field".to_string()))?;

    let raw_offers = match offers {
        Value::Array(items) => items.clone(),
        Value::Object(_) => vec![offers.clone()],
        Value::Null => Vec::new(),
        _ => {
            return Err(ProviderError::InvalidResponse(
                "offers field must be an object or array".to_string(),
            ));
        }
    };

    let observed_at = Utc::now();
    raw_offers
        .into_iter()
        .filter_map(|raw_offer| normalize_offer(raw_offer, observed_at).transpose())
        .filter(|observation| match observation {
            Ok(observation) => spec.matches_observation(observation),
            Err(_) => true,
        })
        .collect()
}

fn normalize_offer(
    raw_offer: Value,
    observed_at: chrono::DateTime<Utc>,
) -> Result<Option<OfferObservation>, ProviderError> {
    let offer: VastOffer = serde_json::from_value(raw_offer.clone())
        .map_err(|error| ProviderError::InvalidResponse(error.to_string()))?;

    let provider_offer_id = offer
        .id
        .or(offer.ask_contract_id)
        .map(|id| id.to_string())
        .ok_or_else(|| ProviderError::InvalidResponse("offer missing id".to_string()))?;

    let gpu_name = match offer.gpu_name {
        Some(gpu_name) if !gpu_name.trim().is_empty() => gpu_name,
        _ => return Ok(None),
    };

    let price_usd_per_hour = match offer.dph_total {
        Some(price) => price,
        None => return Ok(None),
    };

    let verified = offer.verified.unwrap_or_else(|| {
        offer
            .verification
            .as_deref()
            .map(|verification| verification.eq_ignore_ascii_case("verified"))
            .unwrap_or(false)
            || offer.vericode == Some(1)
    });

    let host_id_hash = offer.host_id.map(|host_id| {
        let mut hasher = Sha256::new();
        hasher.update(format!("vast:{host_id}").as_bytes());
        hex::encode(hasher.finalize())
    });

    Ok(Some(OfferObservation {
        observation_id: Uuid::new_v4(),
        observed_at,
        provider: "vast".to_string(),
        provider_offer_id,
        gpu_name,
        num_gpus: offer.num_gpus.unwrap_or(1),
        gpu_ram_gb: offer.gpu_ram.map(|ram_mb| ram_mb / 1024.0),
        price_usd_per_hour,
        reliability_score: offer.reliability.or(offer.reliability2),
        verified,
        rentable: offer.rentable.unwrap_or(false),
        region: offer.geolocation,
        host_id_hash,
        raw_provider_payload: raw_offer,
    }))
}
