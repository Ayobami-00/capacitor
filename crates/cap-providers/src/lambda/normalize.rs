use cap_core::{OfferObservation, WatchSpec};
use chrono::Utc;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::lambda::models::{LambdaInstanceTypeInfo, LambdaInstanceTypesResponse};
use crate::provider::ProviderError;

pub fn normalize_instance_types_response(
    raw: &str,
    spec: &WatchSpec,
) -> Result<Vec<OfferObservation>, ProviderError> {
    let response: LambdaInstanceTypesResponse = serde_json::from_str(raw)
        .map_err(|error| ProviderError::InvalidResponse(error.to_string()))?;
    let value: Value = serde_json::from_str(raw)
        .map_err(|error| ProviderError::InvalidResponse(error.to_string()))?;
    let raw_data = value
        .get("data")
        .and_then(Value::as_object)
        .ok_or_else(|| ProviderError::InvalidResponse("missing data object".to_string()))?;

    let observed_at = Utc::now();
    let mut observations = Vec::new();

    for (instance_type_name, info) in response.data {
        let Some(raw_info) = raw_data.get(&instance_type_name) else {
            continue;
        };

        for region in &info.regions_with_capacity_available {
            if region.name.trim().is_empty() {
                continue;
            }

            let observation =
                normalize_region_offer(&instance_type_name, &info, raw_info, region, observed_at)?;

            if spec.matches_observation(&observation) {
                observations.push(observation);
            }
        }
    }

    Ok(observations)
}

fn normalize_region_offer(
    instance_type_name: &str,
    info: &LambdaInstanceTypeInfo,
    raw_info: &Value,
    region: &crate::lambda::models::LambdaRegion,
    observed_at: chrono::DateTime<Utc>,
) -> Result<OfferObservation, ProviderError> {
    let gpu_name = info
        .instance_type
        .gpu_description
        .as_deref()
        .or(info.instance_type.description.as_deref())
        .unwrap_or(instance_type_name)
        .trim()
        .to_string();

    if gpu_name.is_empty() {
        return Err(ProviderError::InvalidResponse(
            "lambda instance type missing GPU description".to_string(),
        ));
    }

    Ok(OfferObservation {
        observation_id: Uuid::new_v4(),
        observed_at,
        provider: "lambda".to_string(),
        provider_offer_id: format!("{instance_type_name}:{}", region.name),
        gpu_name,
        num_gpus: info.instance_type.specs.gpus.unwrap_or(1),
        gpu_ram_gb: None,
        price_usd_per_hour: info.instance_type.price_cents_per_hour / 100.0,
        reliability_score: Some(1.0),
        verified: true,
        rentable: true,
        region: Some(region.name.clone()),
        host_id_hash: None,
        raw_provider_payload: json!({
            "instance_type_name": instance_type_name,
            "instance_type": raw_info.get("instance_type").cloned().unwrap_or(Value::Null),
            "region": region,
        }),
    })
}
