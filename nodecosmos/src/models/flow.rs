pub mod create;

use crate::api::data::RequestData;
use crate::api::WorkflowParams;
use crate::errors::NodecosmosError;
use crate::models::branch::update::BranchUpdate;
use crate::models::branch::Branch;
use crate::models::flow_step::FlowStep;
use crate::models::traits::{Branchable, FindOrInsertBranchedFromParams};
use charybdis::callbacks::Callbacks;
use charybdis::macros::charybdis_model;
use charybdis::operations::Delete;
use charybdis::types::{Double, Int, Text, Timestamp, Uuid};
use futures::StreamExt;
use nodecosmos_macros::Branchable;
use scylla::CachingSession;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[charybdis_model(
    table_name = flows,
    partition_keys = [node_id, branch_id],
    clustering_keys = [vertical_index, start_index, id],
    local_secondary_indexes = [id]
)]
#[derive(Branchable, Serialize, Deserialize, Default, Clone)]
pub struct Flow {
    #[serde(rename = "nodeId")]
    #[branch(original_id)]
    pub node_id: Uuid,

    #[serde(rename = "branchId")]
    pub branch_id: Uuid,

    // vertical index
    #[serde(rename = "verticalIndex")]
    pub vertical_index: Double,

    // start index is not conflicting, flows can start at same index
    #[serde(rename = "startIndex")]
    pub start_index: Int,

    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,

    pub title: Option<Text>,

    #[serde(rename = "createdAt")]
    pub created_at: Option<Timestamp>,

    #[serde(rename = "updatedAt")]
    pub updated_at: Option<Timestamp>,
}

impl Callbacks for Flow {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_insert(&mut self, _db_session: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        let now = chrono::Utc::now();

        self.id = Uuid::new_v4();
        self.created_at = Some(now);
        self.updated_at = Some(now);

        if self.is_branched() {
            Branch::update(data, self.branch_id, BranchUpdate::EditNodeWorkflow(self.node_id)).await?;
            Branch::update(data, self.branch_id, BranchUpdate::CreateFlow(self.id)).await?;
        }

        Ok(())
    }

    async fn before_delete(&mut self, db_session: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_branched() {
            Branch::update(data, self.branch_id, BranchUpdate::EditNodeWorkflow(self.node_id)).await?;
            Branch::update(data, self.branch_id, BranchUpdate::DeleteFlow(self.id)).await?;
        }

        let flow_steps = self.flow_steps(db_session).await?;

        for mut flow_step in flow_steps {
            flow_step.pull_outputs_from_next_workflow_step(data).await?;
            flow_step.delete_fs_outputs(data).await?;
            flow_step.delete().execute(db_session).await?;
        }

        Ok(())
    }

    async fn after_delete(&mut self, _db_session: &CachingSession, data: &RequestData) -> Result<(), Self::Error> {
        if self.is_branched() {
            self.create_branched_if_original_exists(data).await?;
        }

        Ok(())
    }
}

impl Flow {
    /// merges original and branched flows
    pub async fn branched(db_session: &CachingSession, params: &WorkflowParams) -> Result<Vec<Self>, NodecosmosError> {
        let mut flows = Self::find_by_node_id_and_branch_id(params.node_id, params.branch_id)
            .execute(db_session)
            .await?;

        if params.is_original() {
            Ok(flows.try_collect().await?)
        } else {
            let mut original_flows = Self::find_by_node_id_and_branch_id(params.node_id, params.node_id)
                .execute(db_session)
                .await?;
            let mut branched_flows_set = HashSet::new();
            let mut branch_flows = vec![];

            while let Some(flow) = flows.next().await {
                let flow = flow?;
                branched_flows_set.insert(flow.id);
                branch_flows.push(flow);
            }

            while let Some(flow) = original_flows.next().await {
                let mut flow = flow?;
                if !branched_flows_set.contains(&flow.id) {
                    flow.branch_id = params.branch_id;
                    branch_flows.push(flow);
                }
            }

            branch_flows.sort_by(|a, b| {
                a.vertical_index
                    .partial_cmp(&b.vertical_index)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            Ok(branch_flows)
        }
    }

    pub async fn flow_steps(&self, db_session: &CachingSession) -> Result<Vec<FlowStep>, NodecosmosError> {
        let res = FlowStep::find_by_flow(
            db_session,
            &WorkflowParams {
                node_id: self.node_id,
                branch_id: self.branch_id,
            },
            self.id,
        )
        .await?;

        Ok(res)
    }
}

partial_flow!(
    UpdateTitleFlow,
    node_id,
    branch_id,
    start_index,
    vertical_index,
    id,
    title,
    updated_at
);

impl Callbacks for UpdateTitleFlow {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_update(&mut self, _db_session: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        self.updated_at = Some(chrono::Utc::now());

        if self.is_branched() {
            Branch::update(data, self.branch_id, BranchUpdate::EditNodeWorkflow(self.node_id)).await?;
            Flow::find_or_insert_branched(
                data.db_session(),
                &WorkflowParams {
                    node_id: self.node_id,
                    branch_id: self.branch_id,
                },
                self.id,
            )
            .await?;
            Branch::update(data, self.branch_id, BranchUpdate::EditFlowTitle(self.id)).await?;
        }

        Ok(())
    }
}

partial_flow!(DeleteFlow, node_id, branch_id, start_index, vertical_index, id);
