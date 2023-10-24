use crate::services::resource_locker::ResourceLocker;
use elasticsearch::Elasticsearch;

pub struct CbExtension {
    pub elastic_client: Elasticsearch,
    pub resource_locker: ResourceLocker,
}

impl CbExtension {
    pub fn new(elastic_client: Elasticsearch, resource_locker: ResourceLocker) -> Self {
        Self {
            elastic_client,
            resource_locker,
        }
    }
}
