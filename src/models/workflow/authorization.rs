use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::traits::Authorization;
use crate::models::workflow::Workflow;
use charybdis::types::{Set, Uuid};
use log::error;

impl Authorization for Workflow {
    async fn before_auth(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        self.init_node(data.db_session()).await?;

        Ok(())
    }

    fn is_public(&self) -> bool {
        if let Some(node) = &self.node {
            return node.is_public;
        }

        error!("Workflow {} is missing node {}", self.id, self.node_id);

        false
    }

    fn owner_id(&self) -> Option<Uuid> {
        if let Some(node) = &self.node {
            return node.owner_id;
        }

        error!("Workflow {} is missing node {}", self.id, self.node_id);

        None
    }

    fn editor_ids(&self) -> Option<Set<Uuid>> {
        if let Some(node) = &self.node {
            return node.editor_ids.clone();
        }

        error!("Workflow {} is missing node {}", self.id, self.node_id);

        None
    }

    async fn auth_creation(&mut self, _data: &RequestData) -> Result<(), NodecosmosError> {
        self.auth_update(_data).await
    }
}
