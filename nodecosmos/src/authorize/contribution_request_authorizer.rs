use crate::actions::commit_actions::CommitParams;
use crate::authorize::{auth_node_update, auth_node_update_by_id};
use crate::errors::NodecosmosError;
use crate::models::contribution_request::ContributionRequest;
use crate::models::node::Node;
use crate::models::user::CurrentUser;
use charybdis::operations::{Find, New};
use scylla::CachingSession;

pub async fn auth_contribution_request_creation(
    db_session: &CachingSession,
    contribution_request: &ContributionRequest,
    current_user: &CurrentUser,
) -> Result<(), NodecosmosError> {
    let node = Node {
        id: contribution_request.node_id,
        ..Default::default()
    }
    .find_by_primary_key(db_session)
    .await?;

    if node.is_public.unwrap_or(false) {
        return Ok(());
    }

    auth_node_update(&node, &current_user).await
}

pub async fn auth_contribution_request_update(
    db_session: &CachingSession,
    contribution_request: &ContributionRequest,
    current_user: &CurrentUser,
) -> Result<(), NodecosmosError> {
    if contribution_request.owner_id == Some(current_user.id) {
        Ok(())
    } else {
        auth_node_update_by_id(&contribution_request.node_id, db_session, &current_user).await
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
        auth_node_update_by_id(&contribution_request.node_id, db_session, &current_user).await
    }
}
