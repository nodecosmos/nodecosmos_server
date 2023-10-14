use crate::authorize::auth_node_update_by_id;
use crate::errors::NodecosmosError;
use crate::models::user::CurrentUser;

use charybdis::types::Uuid;
use scylla::CachingSession;

pub async fn auth_workflow_creation(
    db_session: &CachingSession,
    node_id: Uuid,
    current_user: CurrentUser,
) -> Result<(), NodecosmosError> {
    auth_node_update_by_id(&node_id, db_session, &current_user).await
}

pub async fn auth_workflow_update(
    db_session: &CachingSession,
    node_id: Uuid,
    current_user: CurrentUser,
) -> Result<(), NodecosmosError> {
    auth_node_update_by_id(&node_id, db_session, &current_user).await
}
