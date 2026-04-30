use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct VastOffer {
    pub id: Option<i64>,
    pub ask_contract_id: Option<i64>,
    pub gpu_name: Option<String>,
    pub num_gpus: Option<u32>,
    pub gpu_ram: Option<f64>,
    pub dph_total: Option<f64>,
    pub reliability: Option<f64>,
    pub reliability2: Option<f64>,
    pub verified: Option<bool>,
    pub rentable: Option<bool>,
    pub verification: Option<String>,
    pub vericode: Option<i64>,
    pub geolocation: Option<String>,
    pub host_id: Option<i64>,
}
