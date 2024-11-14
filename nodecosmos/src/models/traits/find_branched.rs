use charybdis::model::Model;
use charybdis::stream::CharybdisModelStream;
use charybdis::types::{Set, Uuid};
use futures::TryFutureExt;
use scylla::CachingSession;

use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::description::{find_description, Description};
use crate::models::flow::{find_flow, find_update_title_flow, Flow, UpdateTitleFlow};
use crate::models::flow_step::{
    find_flow_step, find_pk_flow_step, find_update_input_ids_flow_step, find_update_node_ids_flow_step,
    find_update_output_ids_flow_step, FlowStep, PkFlowStep, UpdateInputIdsFlowStep, UpdateNodeIdsFlowStep,
    UpdateOutputIdsFlowStep,
};
use crate::models::node::{BaseNode, GetStructureNode, Node, UpdateTitleNode};
use crate::models::traits::ModelContext;

pub trait FindBranchedOrOriginalNode: Model {
    async fn find_branched_or_original(
        db_session: &CachingSession,
        params: crate::models::traits::NodeBranchParams,
    ) -> Result<Self, NodecosmosError>;
}

macro_rules! impl_find_branched_or_original_node {
    ($struct_name:ident) => {
        impl FindBranchedOrOriginalNode for $struct_name {
            async fn find_branched_or_original(
                db_session: &CachingSession,
                params: crate::models::traits::NodeBranchParams,
            ) -> Result<Self, NodecosmosError> {
                use crate::models::traits::Branchable;

                if params.is_original() {
                    return Self::find_by_branch_id_and_id(params.branch_id, params.node_id)
                        .execute(db_session)
                        .await
                        .map_err(NodecosmosError::from);
                } else {
                    return match Self::maybe_find_first_by_branch_id_and_id(params.branch_id, params.node_id)
                        .execute(db_session)
                        .await?
                    {
                        Some(node) => Ok(node),
                        None => {
                            let mut node = Self::find_by_branch_id_and_id(params.original_id(), params.node_id)
                                .execute(db_session)
                                .await?;
                            node.branch_id = params.branch_id;

                            Ok(node)
                        }
                    };
                }
            }
        }
    };
}

impl_find_branched_or_original_node!(Node);
impl_find_branched_or_original_node!(BaseNode);
impl_find_branched_or_original_node!(GetStructureNode);
impl_find_branched_or_original_node!(UpdateTitleNode);

pub trait FindBranchedOrOriginal: Model {
    async fn find_branched_or_original(
        db_session: &CachingSession,
        params: crate::models::traits::ModelBranchParams,
    ) -> Result<Self, NodecosmosError>;
}

macro_rules! impl_find_branched_or_original {
    ($struct_name:ident) => {
        impl FindBranchedOrOriginal for $struct_name {
            async fn find_branched_or_original(
                db_session: &CachingSession,
                params: crate::models::traits::ModelBranchParams,
            ) -> Result<Self, NodecosmosError> {
                use crate::models::traits::Branchable;

                if params.is_original() {
                    return Self::find_first_by_branch_id_and_id(params.branch_id, params.id)
                        .execute(db_session)
                        .await
                        .map_err(NodecosmosError::from);
                } else {
                    return match Self::maybe_find_first_by_branch_id_and_id(params.branch_id, params.id)
                        .execute(db_session)
                        .await?
                    {
                        Some(model) => Ok(model),
                        None => {
                            let mut model = Self::find_first_by_branch_id_and_id(params.original_id, params.id)
                                .execute(db_session)
                                .await?;
                            model.branch_id = params.branch_id;

                            Ok(model)
                        }
                    };
                }
            }
        }
    };
}

impl_find_branched_or_original!(Flow);
impl_find_branched_or_original!(FlowStep);

pub trait FindOriginalOrBranched: Model {
    async fn find_original_or_branched(
        db_session: &CachingSession,
        params: crate::models::traits::ModelBranchParams,
    ) -> Result<Self, NodecosmosError>;
}

macro_rules! impl_find_original_or_branched {
    ($struct_name:ident) => {
        impl FindOriginalOrBranched for $struct_name {
            async fn find_original_or_branched(
                db_session: &CachingSession,
                params: crate::models::traits::ModelBranchParams,
            ) -> Result<Self, NodecosmosError> {
                use crate::models::traits::Branchable;

                if params.is_original() {
                    return Self::find_first_by_branch_id_and_id(params.branch_id, params.id)
                        .execute(db_session)
                        .await
                        .map_err(NodecosmosError::from);
                } else {
                    return match Self::maybe_find_first_by_branch_id_and_id(params.original_id, params.id)
                        .execute(db_session)
                        .await?
                    {
                        Some(model) => Ok(model),
                        None => {
                            let model = Self::find_first_by_branch_id_and_id(params.branch_id, params.id)
                                .execute(db_session)
                                .await?;

                            Ok(model)
                        }
                    };
                }
            }
        }
    };
}

impl_find_original_or_branched!(FlowStep);

pub trait FindOrInsertBranched: Model {
    async fn find_or_insert_branched(
        data: &RequestData,
        params: crate::models::traits::ModelBranchParams,
    ) -> Result<Self, NodecosmosError>;
}

impl FindOrInsertBranched for Node {
    async fn find_or_insert_branched(
        data: &RequestData,
        params: crate::models::traits::ModelBranchParams,
    ) -> Result<Self, NodecosmosError> {
        use charybdis::operations::{Find, InsertWithCallbacks};

        let node = Self::maybe_find_by_primary_key_value((params.branch_id, params.id))
            .execute(data.db_session())
            .await?;

        match node {
            Some(node) => Ok(node),
            None => {
                let mut node = Self::find_by_primary_key_value((params.original_id, params.id))
                    .execute(data.db_session())
                    .await?;

                node.set_branched_init_context();
                node.branch_id = params.branch_id;

                node.insert_cb(data)
                    .execute(data.db_session())
                    .map_err(|err| err)
                    .await?;

                Ok(node)
            }
        }
    }
}

macro_rules! find_or_insert_branched {
    ($struct:ident) => {
        impl FindOrInsertBranched for $struct {
            async fn find_or_insert_branched(
                data: &RequestData,
                params: crate::models::traits::ModelBranchParams,
            ) -> Result<Self, NodecosmosError> {
                use crate::models::traits::{Branchable, ModelContext};
                use charybdis::operations::InsertWithCallbacks;

                if params.is_original() {
                    return Self::find_first_by_branch_id_and_id(params.branch_id, params.id)
                        .execute(data.db_session())
                        .await
                        .map_err(NodecosmosError::from);
                } else {
                    let maybe_branched = Self::maybe_find_first_by_branch_id_and_id(params.branch_id, params.id)
                        .execute(data.db_session())
                        .await?;

                    if let Some(branched) = maybe_branched {
                        Ok(branched)
                    } else {
                        let mut new_branched = Self::find_first_by_branch_id_and_id(params.original_id, params.id)
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

pub trait FindForBranchMerge: Model {
    async fn find_by_branch_id_and_node_ids(
        db_session: &CachingSession,
        branch_id: Uuid,
        node_ids: &Set<Uuid>,
    ) -> Result<CharybdisModelStream<Self>, NodecosmosError>;

    async fn find_by_branch_id_and_ids(
        db_session: &CachingSession,
        branch_id: Uuid,
        ids: &Set<Uuid>,
    ) -> Result<CharybdisModelStream<Self>, NodecosmosError>;
}

impl FindForBranchMerge for Flow {
    async fn find_by_branch_id_and_node_ids(
        db_session: &CachingSession,
        branch_id: Uuid,
        node_ids: &Set<Uuid>,
    ) -> Result<CharybdisModelStream<Self>, NodecosmosError> {
        find_flow!("branch_id = ? AND node_id IN ?", (branch_id, node_ids))
            .execute(db_session)
            .await
            .map_err(NodecosmosError::from)
    }

    async fn find_by_branch_id_and_ids(
        db_session: &CachingSession,
        branch_id: Uuid,
        ids: &Set<Uuid>,
    ) -> Result<CharybdisModelStream<Self>, NodecosmosError> {
        find_flow!("branch_id = ? AND id IN ? ALLOW FILTERING", (branch_id, ids))
            .execute(db_session)
            .await
            .map_err(NodecosmosError::from)
    }
}

impl FindForBranchMerge for UpdateTitleFlow {
    async fn find_by_branch_id_and_node_ids(
        db_session: &CachingSession,
        branch_id: Uuid,
        node_ids: &Set<Uuid>,
    ) -> Result<CharybdisModelStream<Self>, NodecosmosError> {
        find_update_title_flow!("branch_id = ? AND node_id IN ?", (branch_id, node_ids))
            .execute(db_session)
            .await
            .map_err(NodecosmosError::from)
    }

    async fn find_by_branch_id_and_ids(
        db_session: &CachingSession,
        branch_id: Uuid,
        ids: &Set<Uuid>,
    ) -> Result<CharybdisModelStream<Self>, NodecosmosError> {
        find_update_title_flow!("branch_id = ? AND id IN ? ALLOW FILTERING", (branch_id, ids))
            .execute(db_session)
            .await
            .map_err(NodecosmosError::from)
    }
}

impl FindForBranchMerge for FlowStep {
    async fn find_by_branch_id_and_node_ids(
        db_session: &CachingSession,
        branch_id: Uuid,
        node_ids: &Set<Uuid>,
    ) -> Result<CharybdisModelStream<Self>, NodecosmosError> {
        find_flow_step!("branch_id = ? AND node_id IN ?", (branch_id, node_ids))
            .execute(db_session)
            .await
            .map_err(NodecosmosError::from)
    }

    async fn find_by_branch_id_and_ids(
        db_session: &CachingSession,
        branch_id: Uuid,
        ids: &Set<Uuid>,
    ) -> Result<CharybdisModelStream<Self>, NodecosmosError> {
        find_flow_step!("branch_id = ? AND id IN ? ALLOW FILTERING", (branch_id, ids))
            .execute(db_session)
            .await
            .map_err(NodecosmosError::from)
    }
}

impl FindForBranchMerge for PkFlowStep {
    async fn find_by_branch_id_and_node_ids(
        db_session: &CachingSession,
        branch_id: Uuid,
        node_ids: &Set<Uuid>,
    ) -> Result<CharybdisModelStream<Self>, NodecosmosError> {
        find_pk_flow_step!("branch_id = ? AND node_id IN ?", (branch_id, node_ids))
            .execute(db_session)
            .await
            .map_err(NodecosmosError::from)
    }

    async fn find_by_branch_id_and_ids(
        db_session: &CachingSession,
        branch_id: Uuid,
        ids: &Set<Uuid>,
    ) -> Result<CharybdisModelStream<Self>, NodecosmosError> {
        find_pk_flow_step!("branch_id = ? AND id IN ? ALLOW FILTERING", (branch_id, ids))
            .execute(db_session)
            .await
            .map_err(NodecosmosError::from)
    }
}

impl FindForBranchMerge for UpdateInputIdsFlowStep {
    async fn find_by_branch_id_and_node_ids(
        db_session: &CachingSession,
        branch_id: Uuid,
        node_ids: &Set<Uuid>,
    ) -> Result<CharybdisModelStream<Self>, NodecosmosError> {
        find_update_input_ids_flow_step!("branch_id = ? AND node_id IN ?", (branch_id, node_ids))
            .execute(db_session)
            .await
            .map_err(NodecosmosError::from)
    }

    async fn find_by_branch_id_and_ids(
        db_session: &CachingSession,
        branch_id: Uuid,
        ids: &Set<Uuid>,
    ) -> Result<CharybdisModelStream<Self>, NodecosmosError> {
        find_update_input_ids_flow_step!("branch_id = ? AND id IN ? ALLOW FILTERING", (branch_id, ids))
            .execute(db_session)
            .await
            .map_err(NodecosmosError::from)
    }
}

impl FindForBranchMerge for UpdateOutputIdsFlowStep {
    async fn find_by_branch_id_and_node_ids(
        db_session: &CachingSession,
        branch_id: Uuid,
        node_ids: &Set<Uuid>,
    ) -> Result<CharybdisModelStream<Self>, NodecosmosError> {
        find_update_output_ids_flow_step!("branch_id = ? AND node_id IN ?", (branch_id, node_ids))
            .execute(db_session)
            .await
            .map_err(NodecosmosError::from)
    }

    async fn find_by_branch_id_and_ids(
        db_session: &CachingSession,
        branch_id: Uuid,
        ids: &Set<Uuid>,
    ) -> Result<CharybdisModelStream<Self>, NodecosmosError> {
        find_update_output_ids_flow_step!("branch_id = ? AND id IN ? ALLOW FILTERING", (branch_id, ids))
            .execute(db_session)
            .await
            .map_err(NodecosmosError::from)
    }
}

impl FindForBranchMerge for UpdateNodeIdsFlowStep {
    async fn find_by_branch_id_and_node_ids(
        db_session: &CachingSession,
        branch_id: Uuid,
        node_ids: &Set<Uuid>,
    ) -> Result<CharybdisModelStream<Self>, NodecosmosError> {
        find_update_node_ids_flow_step!("branch_id = ? AND node_id IN ?", (branch_id, node_ids))
            .execute(db_session)
            .await
            .map_err(NodecosmosError::from)
    }

    async fn find_by_branch_id_and_ids(
        db_session: &CachingSession,
        branch_id: Uuid,
        ids: &Set<Uuid>,
    ) -> Result<CharybdisModelStream<Self>, NodecosmosError> {
        find_update_node_ids_flow_step!("branch_id = ? AND id IN ? ALLOW FILTERING", (branch_id, ids))
            .execute(db_session)
            .await
            .map_err(NodecosmosError::from)
    }
}

impl FindForBranchMerge for Description {
    async fn find_by_branch_id_and_node_ids(
        _db_session: &CachingSession,
        _branch_id: Uuid,
        _node_ids: &Set<Uuid>,
    ) -> Result<CharybdisModelStream<Self>, NodecosmosError> {
        unimplemented!("Description is not findable by node_ids");
    }

    async fn find_by_branch_id_and_ids(
        db_session: &CachingSession,
        branch_id: Uuid,
        ids: &Set<Uuid>,
    ) -> Result<CharybdisModelStream<Self>, NodecosmosError> {
        find_description!("branch_id = ? AND object_id IN ?", (branch_id, ids))
            .execute(db_session)
            .await
            .map_err(NodecosmosError::from)
    }
}
