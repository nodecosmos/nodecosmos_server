use crate::app::App;
use crate::errors::NodecosmosError;
use crate::models::node::{Node, UpdateIsSubscriptionActiveNode};
use crate::models::traits::Descendants;
use charybdis::batch::ModelBatch;
use futures::StreamExt;

impl Node {
    pub async fn update_sub_active(&mut self, app: &App, is_sub_active: bool) -> Result<(), NodecosmosError> {
        self.is_subscription_active = is_sub_active;

        let mut descendants = self.descendants(&app.db_session).await?;

        let mut batch = UpdateIsSubscriptionActiveNode::batch();

        batch.append_update_owned(UpdateIsSubscriptionActiveNode {
            root_id: self.root_id,
            branch_id: self.branch_id,
            id: self.id,
            is_subscription_active: is_sub_active,
            updated_at: chrono::Utc::now(),
        });

        while let Some(descendant) = descendants.next().await {
            let descendant = descendant?;

            batch.append_update_owned(UpdateIsSubscriptionActiveNode {
                root_id: descendant.root_id,
                branch_id: descendant.branch_id,
                id: descendant.id,
                is_subscription_active: is_sub_active,
                updated_at: chrono::Utc::now(),
            });
        }

        batch.execute(&app.db_session).await.map_err(|e| {
            log::error!("Failed to update is_subscription_active: {}", e);
            NodecosmosError::InternalServerError(format!("Failed to update is_subscription_active: {}", e))
        })?;

        Ok(())
    }
}
