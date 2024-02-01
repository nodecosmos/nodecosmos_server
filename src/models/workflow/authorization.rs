use crate::api::authorization::Authorization;
use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::workflow::Workflow;
use crate::utils::logger::log_fatal;
use charybdis::types::{Set, Uuid};

impl Authorization for Workflow {
    async fn before_auth(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        self.init_node(data.db_session()).await?;

        Ok(())
    }

    fn is_public(&self) -> bool {
        if let Some(node) = &self.node {
            return node.is_public;
        }

        log_fatal(format!("Workflow {} is missing node {}", self.id, self.node_id));

        false
    }

    fn owner_id(&self) -> Option<Uuid> {
        if let Some(node) = &self.node {
            return node.owner_id;
        }

        log_fatal(format!("Workflow {} is missing node {}", self.id, self.node_id));

        None
    }

    fn editor_ids(&self) -> Option<Set<Uuid>> {
        if let Some(node) = &self.node {
            return node.editor_ids.clone();
        }

        log_fatal(format!("Workflow {} is missing node {}", self.id, self.node_id));

        None
    }

    async fn auth_creation(&mut self, _data: &RequestData) -> Result<(), NodecosmosError> {
        self.auth_update(_data).await
    }
}
