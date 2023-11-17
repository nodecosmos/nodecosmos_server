use crate::services::elastic::utils::idx_exists;
use colored::Colorize;
use elasticsearch::indices::{IndicesCreateParts, IndicesPutMappingParts};
use elasticsearch::Elasticsearch;
use serde_json::{json, Value};

pub trait ElasticIndex {
    const ELASTIC_IDX_NAME: &'static str;

    fn settings_json() -> Value;
    fn mappings_json() -> Value;
}

pub trait BuildIndex: ElasticIndex {
    async fn build_index(client: &Elasticsearch);
}

impl<T: ElasticIndex> BuildIndex for T {
    async fn build_index(client: &Elasticsearch) {
        let response;

        if idx_exists(client, Self::ELASTIC_IDX_NAME).await {
            println!(
                "{} {}",
                "Sync elastic index for".bright_green(),
                Self::ELASTIC_IDX_NAME.bright_yellow()
            );

            response = client
                .indices()
                .put_mapping(IndicesPutMappingParts::Index(&[Self::ELASTIC_IDX_NAME]))
                .body(Self::mappings_json())
                .send()
                .await;
        } else {
            println!(
                "{} {}",
                "Creating elastic index for".bright_green(),
                Self::ELASTIC_IDX_NAME.bright_yellow()
            );
            response = client
                .indices()
                .create(IndicesCreateParts::Index(Self::ELASTIC_IDX_NAME))
                .body(json!({
                    "settings": Self::mappings_json(),
                    "mappings": Self::settings_json()
                }))
                .send()
                .await;
        }

        let response = response.unwrap_or_else(|e| {
            panic!("Failed to handle node index: {:#?}", e);
        });

        if !response.status_code().is_success() {
            eprintln!("Failed Elasticsearch operation. Debug info: ...");

            panic!(
                "Failed to handle node index: {}! Response body: {}",
                response.status_code(),
                response.text().await.unwrap_or("No Body!".to_string()).bright_red()
            );
        }
    }
}
