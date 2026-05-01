mod client;
mod models;
mod normalize;

pub use client::LambdaProvider;

#[cfg(test)]
mod tests {
    use cap_core::WatchSpec;

    use crate::lambda::normalize::normalize_instance_types_response;

    #[test]
    fn normalizes_available_lambda_regions() {
        let raw = include_str!("../../../../fixtures/lambda/instance_types_sample.json");
        let spec = WatchSpec {
            provider: "lambda".to_string(),
            gpu_filters: vec!["H100".to_string()],
            max_price: Some(36.0),
            verified: true,
            min_reliability: Some(0.98),
            min_gpus: Some(8),
            poll_interval_secs: 60,
        };

        let observations = normalize_instance_types_response(raw, &spec).unwrap();

        assert_eq!(observations.len(), 2);
        assert!(observations.iter().all(|observation| {
            observation.provider == "lambda"
                && observation.gpu_name == "NVIDIA H100 SXM"
                && observation.num_gpus == 8
                && observation.verified
                && observation.reliability_score == Some(1.0)
                && observation.price_usd_per_hour == 31.92
        }));
        assert_eq!(
            observations[0].provider_offer_id,
            "gpu_8x_h100_sxm5:us-east-1"
        );
    }

    #[test]
    fn ignores_lambda_types_without_available_regions() {
        let raw = include_str!("../../../../fixtures/lambda/instance_types_no_capacity.json");
        let spec = WatchSpec {
            provider: "lambda".to_string(),
            gpu_filters: vec!["H100".to_string()],
            max_price: Some(36.0),
            verified: true,
            min_reliability: Some(0.98),
            min_gpus: Some(8),
            poll_interval_secs: 60,
        };

        let observations = normalize_instance_types_response(raw, &spec).unwrap();

        assert!(observations.is_empty());
    }

    #[test]
    fn filters_lambda_observations_with_existing_watch_spec() {
        let raw = include_str!("../../../../fixtures/lambda/instance_types_sample.json");
        let spec = WatchSpec {
            provider: "lambda".to_string(),
            gpu_filters: vec!["A100".to_string()],
            max_price: Some(20.0),
            verified: true,
            min_reliability: Some(0.98),
            min_gpus: Some(4),
            poll_interval_secs: 60,
        };

        let observations = normalize_instance_types_response(raw, &spec).unwrap();

        assert_eq!(observations.len(), 1);
        assert_eq!(observations[0].gpu_name, "NVIDIA A100 PCIe");
        assert_eq!(observations[0].num_gpus, 4);
    }
}
