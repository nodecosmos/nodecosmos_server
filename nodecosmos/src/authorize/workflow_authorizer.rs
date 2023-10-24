use crate::authorize::{auth_node_update, auth_node_update_by_id};
use crate::errors::NodecosmosError;
use crate::models::user::CurrentUser;

use crate::models::input_output::InputOutput;
use crate::models::node::Node;
use crate::models::workflow::Workflow;
use charybdis::operations::Find;
use charybdis::types::Uuid;
use scylla::CachingSession;
use serde_json::json;

pub async fn auth_workflow_creation(
    db_session: &CachingSession,
    workflow: &Workflow,
    current_user: &CurrentUser,
) -> Result<(), NodecosmosError> {
    let node = Node {
        id: workflow.node_id,
        ..Default::default()
    }
    .find_by_primary_key(&db_session)
    .await?;

    auth_node_update(&node, &current_user).await?;

    // check if root node id matches
    if workflow.root_node_id != node.root_id {
        return Err(NodecosmosError::Unauthorized(json!({
            "error": "Unauthorized",
            "message": "Not authorized to add workflow for this node!"
        })));
    }

    Ok(())
}

pub async fn auth_workflow_update(
    db_session: &CachingSession,
    node_id: Uuid,
    current_user: CurrentUser,
) -> Result<(), NodecosmosError> {
    auth_node_update_by_id(&node_id, db_session, &current_user).await
}

pub async fn auth_input_output_creation(
    db_session: &CachingSession,
    io: &InputOutput,
    current_user: &CurrentUser,
) -> Result<(), NodecosmosError> {
    let node = Node {
        id: io.node_id,
        ..Default::default()
    }
    .find_by_primary_key(&db_session)
    .await?;

    auth_node_update(&node, &current_user).await?;

    // check if root node id matches
    if io.root_node_id != node.root_id {
        return Err(NodecosmosError::Unauthorized(json!({
            "error": "Unauthorized",
            "message": "Not authorized to add workflow for this node!"
        })));
    }

    Ok(())
}
