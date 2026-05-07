use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RunpodGraphqlResponse {
    pub data: Option<RunpodGpuTypesData>,
    #[serde(default)]
    pub errors: Vec<RunpodGraphqlError>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunpodGpuTypesData {
    pub gpu_types: Vec<RunpodGpuType>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RunpodGraphqlError {
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunpodGpuType {
    pub id: String,
    pub display_name: Option<String>,
    pub memory_in_gb: Option<f64>,
    pub lowest_price: Option<RunpodLowestPrice>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunpodLowestPrice {
    pub gpu_name: Option<String>,
    pub gpu_type_id: Option<String>,
    pub uninterruptable_price: Option<f64>,
    pub stock_status: Option<String>,
    pub country_code: Option<String>,
    #[serde(default)]
    pub available_gpu_counts: Option<Vec<u32>>,
}
