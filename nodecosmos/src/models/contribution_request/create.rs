use charybdis::operations::{Insert, InsertWithCallbacks};
use charybdis::types::Uuid;
use std::collections::HashSet;

use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::Branch;
use crate::models::contribution_request::ContributionRequest;
use crate::models::traits::ModelContext;

impl ContributionRequest {
    pub fn set_defaults(&mut self, data: &RequestData) {
        let now = chrono::Utc::now();

        self.id = Uuid::new_v4();
        self.created_at = now;
        self.updated_at = now;

        self.owner_id = data.current_user.id;
        self.owner = Some((&data.current_user).into());
    }

    pub async fn create_branch_node(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let mut node = self.node(data.db_session()).await?.clone();

        node.branch_id = self.id;

        node.set_branched_init_context();
        node.insert_cb(data).execute(data.db_session()).await?;

        Ok(())
    }

    pub async fn create_branch(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let node = self.node(data.db_session()).await?;
        let root_id = node.root_id;
        let is_public = node.is_public;

        let mut editor_ids = HashSet::new();

        if let Some(ids) = &node.editor_ids {
            editor_ids = ids.clone();
        }

        editor_ids.insert(node.owner_id);

        let branch = Branch {
            id: self.id, //
            node_id: self.node_id,
            root_id,
            title: self.title.clone(),
            description: self.description.clone(),
            owner_id: self.owner_id,
            owner: self.owner.clone(),
            is_public,
            is_contribution_request: Some(true),
            editor_ids: Some(editor_ids),
            ..Default::default()
        };

        branch.insert().execute(data.db_session()).await?;

        Ok(())
    }
}
