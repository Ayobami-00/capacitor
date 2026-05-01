use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LambdaInstanceTypesResponse {
    pub data: BTreeMap<String, LambdaInstanceTypeInfo>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LambdaInstanceTypeInfo {
    pub instance_type: LambdaInstanceType,
    pub regions_with_capacity_available: Vec<LambdaRegion>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LambdaInstanceType {
    pub name: Option<String>,
    pub description: Option<String>,
    pub gpu_description: Option<String>,
    pub price_cents_per_hour: f64,
    pub specs: LambdaInstanceTypeSpecs,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LambdaInstanceTypeSpecs {
    pub vcpus: Option<u32>,
    pub memory_gib: Option<u32>,
    pub storage_gib: Option<u32>,
    pub gpus: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LambdaRegion {
    pub name: String,
    pub description: Option<String>,
}
