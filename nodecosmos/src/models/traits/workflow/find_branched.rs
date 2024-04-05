use crate::api::data::RequestData;
use crate::api::WorkflowParams;
use crate::errors::NodecosmosError;
use crate::models::flow::{find_flow, find_update_title_flow, Flow, UpdateTitleFlow};
use crate::models::flow_step::{
    find_flow_step, find_update_input_ids_flow_step, find_update_node_ids_flow_step, find_update_output_ids_flow_step,
    FlowStep, SiblingFlowStep, UpdateInputIdsFlowStep, UpdateNodeIdsFlowStep, UpdateOutputIdsFlowStep,
};
use crate::models::node::Node;
use crate::models::traits::ModelContext;
use charybdis::model::Model;
use charybdis::stream::CharybdisModelStream;
use charybdis::types::{Set, Uuid};
use scylla::CachingSession;

pub trait FindOrInsertBranchedFromParams: Model {
    async fn find_or_insert_branched(
        data: &RequestData,
        params: &WorkflowParams,
        id: Uuid,
    ) -> Result<Self, NodecosmosError>;
}

impl FindOrInsertBranchedFromParams for Node {
    async fn find_or_insert_branched(
        data: &RequestData,
        params: &WorkflowParams,
        id: Uuid,
    ) -> Result<Self, NodecosmosError> {
        use crate::models::traits::Branchable;
        use charybdis::operations::InsertWithCallbacks;

        if params.is_original() {
            return Self::find_first_by_id_and_branch_id(id, id)
                .execute(data.db_session())
                .await
                .map_err(NodecosmosError::from);
        } else {
            let maybe_branched = Self::maybe_find_first_by_id_and_branch_id(id, params.branch_id)
                .execute(data.db_session())
                .await?;

            if let Some(branched) = maybe_branched {
                Ok(branched)
            } else {
                let mut new_branched = Self::find_first_by_id_and_branch_id(id, id)
                    .execute(data.db_session())
                    .await?;

                new_branched.branch_id = params.branch_id;
                new_branched.set_branched_init_context();

                new_branched.insert_cb(data).execute(data.db_session()).await?;

                Ok(new_branched)
            }
        }
    }
}

macro_rules! find_or_insert_branched {
    ($struct:ident) => {
        impl FindOrInsertBranchedFromParams for $struct {
            async fn find_or_insert_branched(
                data: &RequestData,
                params: &WorkflowParams,
                id: charybdis::types::Uuid,
            ) -> Result<Self, NodecosmosError> {
                use crate::models::traits::Branchable;
                use crate::models::traits::ModelContext;
                use charybdis::operations::InsertWithCallbacks;

                if params.is_original() {
                    return Self::find_first_by_node_id_and_branch_id_and_id(params.node_id, params.branch_id, id)
                        .execute(data.db_session())
                        .await
                        .map_err(NodecosmosError::from);
                } else {
                    let maybe_branched =
                        Self::maybe_find_first_by_node_id_and_branch_id_and_id(params.node_id, params.branch_id, id)
                            .execute(data.db_session())
                            .await?;

                    if let Some(branched) = maybe_branched {
                        Ok(branched)
                    } else {
                        let mut new_branched =
                            Self::find_first_by_node_id_and_branch_id_and_id(params.node_id, params.node_id, id)
                                .execute(data.db_session())
                                .await?;

                        new_branched.branch_id = params.branch_id;
                        new_branched.set_branched_init_context();

                        new_branched.insert_cb(data).execute(data.db_session()).await?;

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

pub trait FindForBranchMerge: Model {
    async fn find_by_node_ids_and_branch_id_and_ids(
        db_session: &CachingSession,
        node_ids: &Set<Uuid>,
        branch_id: Uuid,
        ids: &Set<Uuid>,
    ) -> Result<Vec<Self>, NodecosmosError>;

    async fn find_original_by_ids(
        db_session: &CachingSession,
        node_ids: &Set<Uuid>,
        ids: &Set<Uuid>,
    ) -> Result<CharybdisModelStream<Self>, NodecosmosError>;
}

impl FindForBranchMerge for Flow {
    async fn find_by_node_ids_and_branch_id_and_ids(
        db_session: &CachingSession,
        node_ids: &Set<Uuid>,
        branch_id: Uuid,
        ids: &Set<Uuid>,
    ) -> Result<Vec<Self>, NodecosmosError> {
        let flows = find_flow!(
            "node_id IN ? AND branch_id = ? AND id IN ? ALLOW FILTERING",
            (node_ids, branch_id, ids)
        )
        .execute(db_session)
        .await?
        .try_collect()
        .await?;

        Ok(flows)
    }

    async fn find_original_by_ids(
        db_session: &CachingSession,
        node_ids: &Set<Uuid>,
        ids: &Set<Uuid>,
    ) -> Result<CharybdisModelStream<Self>, NodecosmosError> {
        let flows = find_flow!(
            "node_id IN ? AND branch_id IN ? AND id IN ? ALLOW FILTERING",
            (node_ids, node_ids, ids)
        )
        .execute(db_session)
        .await?;

        Ok(flows)
    }
}

impl FindForBranchMerge for UpdateTitleFlow {
    async fn find_by_node_ids_and_branch_id_and_ids(
        db_session: &CachingSession,
        node_ids: &Set<Uuid>,
        branch_id: Uuid,
        ids: &Set<Uuid>,
    ) -> Result<Vec<Self>, NodecosmosError> {
        let flows = find_update_title_flow!(
            "node_id IN ? AND branch_id = ? AND id IN ? ALLOW FILTERING",
            (node_ids, branch_id, ids)
        )
        .execute(db_session)
        .await?
        .try_collect()
        .await?;

        Ok(flows)
    }

    async fn find_original_by_ids(
        db_session: &CachingSession,
        node_ids: &Set<Uuid>,
        ids: &Set<Uuid>,
    ) -> Result<CharybdisModelStream<Self>, NodecosmosError> {
        let flows = find_update_title_flow!(
            "node_id IN ? AND branch_id IN ? AND id IN ? ALLOW FILTERING",
            (node_ids, node_ids, ids)
        )
        .execute(db_session)
        .await?;

        Ok(flows)
    }
}

impl FindForBranchMerge for FlowStep {
    async fn find_by_node_ids_and_branch_id_and_ids(
        db_session: &CachingSession,
        node_ids: &Set<Uuid>,
        branch_id: Uuid,
        ids: &Set<Uuid>,
    ) -> Result<Vec<Self>, NodecosmosError> {
        let flows = find_flow_step!(
            "node_id IN ? AND branch_id = ? AND id IN ? ALLOW FILTERING",
            (node_ids, branch_id, ids)
        )
        .execute(db_session)
        .await?
        .try_collect()
        .await?;

        Ok(flows)
    }

    async fn find_original_by_ids(
        db_session: &CachingSession,
        node_ids: &Set<Uuid>,
        ids: &Set<Uuid>,
    ) -> Result<CharybdisModelStream<Self>, NodecosmosError> {
        let flows = find_flow_step!(
            "node_id IN ? AND branch_id IN ? AND id IN ? ALLOW FILTERING",
            (node_ids, node_ids, ids)
        )
        .execute(db_session)
        .await?;

        Ok(flows)
    }
}

impl FindForBranchMerge for UpdateInputIdsFlowStep {
    async fn find_by_node_ids_and_branch_id_and_ids(
        db_session: &CachingSession,
        node_ids: &Set<Uuid>,
        branch_id: Uuid,
        ids: &Set<Uuid>,
    ) -> Result<Vec<Self>, NodecosmosError> {
        let flows = find_update_input_ids_flow_step!(
            "node_id IN ? AND branch_id = ? AND id IN ? ALLOW FILTERING",
            (node_ids, branch_id, ids)
        )
        .execute(db_session)
        .await?
        .try_collect()
        .await?;

        Ok(flows)
    }

    async fn find_original_by_ids(
        db_session: &CachingSession,
        node_ids: &Set<Uuid>,
        ids: &Set<Uuid>,
    ) -> Result<CharybdisModelStream<Self>, NodecosmosError> {
        let flows = find_update_input_ids_flow_step!(
            "node_id IN ? AND branch_id IN ? AND id IN ? ALLOW FILTERING",
            (node_ids, node_ids, ids)
        )
        .execute(db_session)
        .await?;

        Ok(flows)
    }
}

impl FindForBranchMerge for UpdateOutputIdsFlowStep {
    async fn find_by_node_ids_and_branch_id_and_ids(
        db_session: &CachingSession,
        node_ids: &Set<Uuid>,
        branch_id: Uuid,
        ids: &Set<Uuid>,
    ) -> Result<Vec<Self>, NodecosmosError> {
        let flows = find_update_output_ids_flow_step!(
            "node_id IN ? AND branch_id = ? AND id IN ? ALLOW FILTERING",
            (node_ids, branch_id, ids)
        )
        .execute(db_session)
        .await?
        .try_collect()
        .await?;

        Ok(flows)
    }

    async fn find_original_by_ids(
        db_session: &CachingSession,
        node_ids: &Set<Uuid>,
        ids: &Set<Uuid>,
    ) -> Result<CharybdisModelStream<Self>, NodecosmosError> {
        let flows = find_update_output_ids_flow_step!(
            "node_id IN ? AND branch_id IN ? AND id IN ? ALLOW FILTERING",
            (node_ids, node_ids, ids)
        )
        .execute(db_session)
        .await?;

        Ok(flows)
    }
}

impl FindForBranchMerge for UpdateNodeIdsFlowStep {
    async fn find_by_node_ids_and_branch_id_and_ids(
        db_session: &CachingSession,
        node_ids: &Set<Uuid>,
        branch_id: Uuid,
        ids: &Set<Uuid>,
    ) -> Result<Vec<Self>, NodecosmosError> {
        let flows = find_update_node_ids_flow_step!(
            "node_id IN ? AND branch_id = ? AND id IN ? ALLOW FILTERING",
            (node_ids, branch_id, ids)
        )
        .execute(db_session)
        .await?
        .try_collect()
        .await?;

        Ok(flows)
    }

    async fn find_original_by_ids(
        db_session: &CachingSession,
        node_ids: &Set<Uuid>,
        ids: &Set<Uuid>,
    ) -> Result<CharybdisModelStream<Self>, NodecosmosError> {
        let flows = find_update_node_ids_flow_step!(
            "node_id IN ? AND branch_id IN ? AND id IN ? ALLOW FILTERING",
            (node_ids, node_ids, ids)
        )
        .execute(db_session)
        .await?;

        Ok(flows)
    }
}
