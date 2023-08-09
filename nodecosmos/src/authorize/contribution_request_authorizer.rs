use crate::actions::client_session::CurrentUser;
use crate::actions::commit_actions::CommitParams;
use crate::authorize::can_edit_node;
use crate::errors::NodecosmosError;
use crate::models::contribution_request::ContributionRequest;
use crate::models::node::{find_node_query, Node};
use charybdis::{Find, New};
use scylla::CachingSession;
use serde_json::json;

pub async fn auth_contribution_request_creation(
    db_session: &CachingSession,
    contribution_request: &ContributionRequest,
    current_user: &CurrentUser,
) -> Result<(), NodecosmosError> {
    let node = Node::find_one(
        db_session,
        find_node_query!("id = ?"),
        (contribution_request.node_id,),
    )
    .await?;

    if node.is_public.unwrap_or(false) || can_edit_node(current_user, &node) {
        Ok(())
    } else {
        Err(NodecosmosError::Unauthorized(json!({
            "error": "Unauthorized",
            "message": "Not authorized to create contribution request for provided node!"
        })))
    }
}

pub async fn auth_contribution_request_update(
    db_session: &CachingSession,
    contribution_request: &ContributionRequest,
    current_user: &CurrentUser,
) -> Result<(), NodecosmosError> {
    if contribution_request.owner_id == Some(current_user.id) {
        Ok(())
    } else {
        let node = Node::find_one(
            db_session,
            find_node_query!("id = ?"),
            (contribution_request.node_id,),
        )
        .await?;

        if can_edit_node(current_user, &node) {
            return Ok(());
        }

        Err(NodecosmosError::Unauthorized(json!({
            "error": "Unauthorized",
            "message": "Not authorized to update current contribution request!"
        })))
    }
}

pub async fn auth_commit(
    db_session: &CachingSession,
    params: &CommitParams,
    current_user: &CurrentUser,
) -> Result<(), NodecosmosError> {
    let mut contribution_request = ContributionRequest::new();
    contribution_request.node_id = params.node_id;
    contribution_request.id = params.contribution_request_id;

    let contribution_request = contribution_request.find_by_primary_key(db_session).await?;

    if contribution_request.owner_id == Some(current_user.id) {
        Ok(())
    } else {
        let node = Node::find_one(
            db_session,
            find_node_query!("id = ?"),
            (contribution_request.node_id,),
        )
        .await?;
        if can_edit_node(current_user, &node) {
            Ok(())
        } else {
            Err(NodecosmosError::Unauthorized(json!({
                "error": "Unauthorized",
                "message": "Not authorized to commit to current contribution request!"
            })))
        }
    }
}
