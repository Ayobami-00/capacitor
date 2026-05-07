use cap_core::{OfferObservation, WatchSpec};
use chrono::Utc;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::provider::ProviderError;
use crate::runpod::models::{RunpodGpuType, RunpodGraphqlResponse, RunpodLowestPrice};

pub fn normalize_gpu_types_response(
    raw: &str,
    spec: &WatchSpec,
) -> Result<Vec<OfferObservation>, ProviderError> {
    let response: RunpodGraphqlResponse = serde_json::from_str(raw)
        .map_err(|error| ProviderError::InvalidResponse(error.to_string()))?;
    if !response.errors.is_empty() {
        let message = response
            .errors
            .into_iter()
            .map(|error| error.message)
            .collect::<Vec<_>>()
            .join("; ");
        return Err(ProviderError::InvalidResponse(message));
    }

    let value: Value = serde_json::from_str(raw)
        .map_err(|error| ProviderError::InvalidResponse(error.to_string()))?;
    let raw_gpu_types = value
        .pointer("/data/gpuTypes")
        .and_then(Value::as_array)
        .ok_or_else(|| ProviderError::InvalidResponse("missing data.gpuTypes array".to_string()))?;
    let data = response
        .data
        .ok_or_else(|| ProviderError::InvalidResponse("missing data object".to_string()))?;

    let observed_at = Utc::now();
    let requested_gpu_count = spec.min_gpus.unwrap_or(1).max(1);
    let mut observations = Vec::new();

    for (index, gpu_type) in data.gpu_types.iter().enumerate() {
        let raw_gpu_type = raw_gpu_types.get(index).unwrap_or(&Value::Null);
        let Some(observation) =
            normalize_gpu_type(gpu_type, raw_gpu_type, requested_gpu_count, observed_at)?
        else {
            continue;
        };

        if spec.matches_observation(&observation) {
            observations.push(observation);
        }
    }

    Ok(observations)
}

fn normalize_gpu_type(
    gpu_type: &RunpodGpuType,
    raw_gpu_type: &Value,
    requested_gpu_count: u32,
    observed_at: chrono::DateTime<Utc>,
) -> Result<Option<OfferObservation>, ProviderError> {
    let Some(lowest_price) = gpu_type.lowest_price.as_ref() else {
        return Ok(None);
    };

    if !is_rentable(lowest_price, requested_gpu_count) {
        return Ok(None);
    }

    let Some(unit_price) = lowest_price.uninterruptable_price else {
        return Ok(None);
    };

    let gpu_name = gpu_type
        .display_name
        .as_deref()
        .or(lowest_price.gpu_name.as_deref())
        .unwrap_or(&gpu_type.id)
        .trim()
        .to_string();
    if gpu_name.is_empty() {
        return Err(ProviderError::InvalidResponse(
            "runpod GPU type missing display name".to_string(),
        ));
    }

    let region = lowest_price
        .country_code
        .as_deref()
        .filter(|country| !country.trim().is_empty())
        .unwrap_or("Runpod Secure Cloud")
        .to_string();

    Ok(Some(OfferObservation {
        observation_id: Uuid::new_v4(),
        observed_at,
        provider: "runpod".to_string(),
        provider_offer_id: format!("{}:secure:{requested_gpu_count}:{region}", gpu_type.id),
        gpu_name,
        num_gpus: requested_gpu_count,
        gpu_ram_gb: gpu_type.memory_in_gb,
        price_usd_per_hour: unit_price * f64::from(requested_gpu_count),
        reliability_score: Some(1.0),
        verified: true,
        rentable: true,
        region: Some(region),
        host_id_hash: None,
        raw_provider_payload: json!({
            "gpu_type": raw_gpu_type,
            "lowest_price": lowest_price,
            "secure_cloud": true,
            "requested_gpu_count": requested_gpu_count,
        }),
    }))
}

fn is_rentable(lowest_price: &RunpodLowestPrice, requested_gpu_count: u32) -> bool {
    if lowest_price
        .stock_status
        .as_deref()
        .is_some_and(|status| status.eq_ignore_ascii_case("none"))
    {
        return false;
    }

    if lowest_price.uninterruptable_price.is_none() {
        return false;
    }

    let available_gpu_counts = lowest_price.available_gpu_counts.as_deref().unwrap_or(&[]);

    available_gpu_counts.is_empty() || available_gpu_counts.contains(&requested_gpu_count)
}
