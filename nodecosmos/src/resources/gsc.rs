use reqwest::Client;
use std::sync::{Arc, Mutex};

pub struct GCSConfig {
    pub project_id: String,
    pub bucket: String,
    pub client: Client,
    pub auth: Arc<Mutex<ServiceAccountAccess>>,
}
