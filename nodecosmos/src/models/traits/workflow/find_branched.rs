use crate::api::WorkflowParams;
use crate::errors::NodecosmosError;
use crate::models::flow::Flow;
use crate::models::flow_step::{FlowStep, SiblingFlowStep};
use charybdis::model::Model;
use scylla::CachingSession;

pub trait FindOrInsertBranchedFromParams: Model {
    async fn find_or_insert_branched(
        db_session: &CachingSession,
        params: &WorkflowParams,
        id: charybdis::types::Uuid,
    ) -> Result<Self, NodecosmosError>;
}

macro_rules! find_or_insert_branched {
    ($struct:ident) => {
        impl FindOrInsertBranchedFromParams for $struct {
            async fn find_or_insert_branched(
                db_session: &CachingSession,
                params: &WorkflowParams,
                id: charybdis::types::Uuid,
            ) -> Result<Self, NodecosmosError> {
                use crate::models::traits::Branchable;
                use charybdis::operations::Insert;

                if params.is_original() {
                    return Self::find_first_by_node_id_and_branch_id_and_id(params.node_id, params.branch_id, id)
                        .execute(db_session)
                        .await
                        .map_err(NodecosmosError::from);
                } else {
                    let maybe_branched =
                        Self::maybe_find_first_by_node_id_and_branch_id_and_id(params.node_id, params.branch_id, id)
                            .execute(db_session)
                            .await?;

                    if let Some(branched) = maybe_branched {
                        Ok(branched)
                    } else {
                        let mut new_branched =
                            Self::find_first_by_node_id_and_branch_id_and_id(params.node_id, params.node_id, id)
                                .execute(db_session)
                                .await?;

                        new_branched.branch_id = params.branch_id;

                        new_branched.insert().execute(db_session).await?;

                        Ok(new_branched)
                    }
                }
            }
        }
    };
}

find_or_insert_branched!(Flow);
find_or_insert_branched!(FlowStep);
find_or_insert_branched!(SiblingFlowStep);
