use crate::actions::client_session::CurrentUser;
use crate::authorize::auth_node_update;
use crate::errors::NodecosmosError;
use crate::models::node::{find_node_query, Node};
use crate::models::workflow::Workflow;

use charybdis::{Find, Uuid};
use scylla::CachingSession;

pub async fn auth_workflow_creation(
    db_session: &CachingSession,
    node_id: Uuid,
    current_user: CurrentUser,
) -> Result<(), NodecosmosError> {
    let node = Node::find_one(&db_session, find_node_query!("id = ?"), (node_id,)).await?;

    auth_node_update(&node, &current_user).await?;

    Ok(())
}

pub async fn auth_workflow_update(
    db_session: &CachingSession,
    node_id: Uuid,
    workflow_id: Uuid,
    current_user: CurrentUser,
) -> Result<(), NodecosmosError> {
    let workflow = Workflow {
        node_id,
        id: workflow_id,
        ..Default::default()
    };

    workflow.find_by_primary_key(&db_session).await?;

    println!("{}", find_node_query!("id = ?"));

    let node = Node::find_one(&db_session, find_node_query!("id = ?"), (node_id,)).await?;

    auth_node_update(&node, &current_user).await?;

    Ok(())
}
