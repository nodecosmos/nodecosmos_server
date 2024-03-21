pub mod callbacks;
pub mod create;

use crate::api::WorkflowParams;
use crate::errors::NodecosmosError;
use crate::models::flow_step::FlowStep;
use crate::models::traits::Branchable;
use crate::models::workflow::Workflow;
use charybdis::macros::charybdis_model;
use charybdis::stream::CharybdisModelStream;
use charybdis::types::{Double, Int, Text, Timestamp, Uuid};
use futures::StreamExt;
use nodecosmos_macros::BranchableNodeId;
use scylla::CachingSession;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[charybdis_model(
    table_name = flows,
    partition_keys = [node_id, branch_id],
    clustering_keys = [workflow_id, vertical_index, start_index, id],
    local_secondary_indexes = [id]
)]
#[derive(BranchableNodeId, Serialize, Deserialize, Default)]
pub struct Flow {
    #[serde(rename = "nodeId")]
    pub node_id: Uuid,

    #[serde(rename = "branchId")]
    pub branch_id: Uuid,

    #[serde(rename = "workflowId")]
    pub workflow_id: Uuid,

    // vertical index
    #[serde(rename = "verticalIndex")]
    pub vertical_index: Double,

    // start index is not conflicting, flows can start at same index
    #[serde(rename = "startIndex")]
    pub start_index: Int,

    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,

    pub title: Option<Text>,
    pub description: Option<Text>,

    #[serde(rename = "descriptionMarkdown")]
    pub description_markdown: Option<Text>,

    #[serde(rename = "descriptionBase64")]
    pub description_base64: Option<Text>,

    #[serde(rename = "createdAt")]
    pub created_at: Option<Timestamp>,

    #[serde(rename = "updatedAt")]
    pub updated_at: Option<Timestamp>,

    #[serde(skip)]
    #[charybdis(ignore)]
    pub workflow: Option<Workflow>,
}

impl Flow {
    pub async fn find_by_node_id_and_branch_id_and_id(
        session: &CachingSession,
        node_id: Uuid,
        branch_id: Uuid,
        id: Uuid,
    ) -> Result<Flow, NodecosmosError> {
        let flow = find_first_flow!("node_id = ? AND branch_id = ? AND id = ?", (node_id, branch_id, id))
            .execute(session)
            .await?;

        Ok(flow)
    }

    pub async fn flow_steps(
        &self,
        session: &CachingSession,
    ) -> Result<CharybdisModelStream<FlowStep>, NodecosmosError> {
        let res = FlowStep::find_by_flow(session, self.node_id, self.branch_id, self.workflow_id, self.id).await?;

        Ok(res)
    }

    pub async fn workflow(&mut self, session: &CachingSession) -> Result<&mut Workflow, NodecosmosError> {
        if self.workflow.is_none() {
            let params = WorkflowParams {
                node_id: self.node_id,
                branch_id: self.branch_id,
            };

            let workflow = Workflow::branched(session, &params).await?;
            self.workflow = Some(workflow);
        }

        Ok(self.workflow.as_mut().unwrap())
    }
}

partial_flow!(
    BaseFlow,
    node_id,
    branch_id,
    workflow_id,
    start_index,
    vertical_index,
    id,
    title,
    created_at,
    updated_at
);

impl BaseFlow {
    /// merges original and branched flows
    pub async fn branched(session: &CachingSession, params: &WorkflowParams) -> Result<Vec<BaseFlow>, NodecosmosError> {
        let mut flows = BaseFlow::find_by_node_id_and_branch_id(params.node_id, params.branch_id)
            .execute(session)
            .await?;

        if params.is_original() {
            Ok(flows.try_collect().await?)
        } else {
            let mut original_flows = BaseFlow::find_by_node_id_and_branch_id(params.node_id, params.node_id)
                .execute(session)
                .await?;
            let mut branched_flows_set = HashSet::new();
            let mut branch_flows = vec![];

            while let Some(flow) = flows.next().await {
                let flow = flow?;
                branched_flows_set.insert(flow.id);
                branch_flows.push(flow);
            }

            while let Some(flow) = original_flows.next().await {
                let flow = flow?;
                if !branched_flows_set.contains(&flow.id) {
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
}

partial_flow!(
    UpdateTitleFlow,
    node_id,
    branch_id,
    workflow_id,
    start_index,
    vertical_index,
    id,
    title,
    updated_at
);

partial_flow!(
    DescriptionFlow,
    node_id,
    branch_id,
    workflow_id,
    start_index,
    vertical_index,
    id,
    description,
    description_markdown,
    updated_at
);

partial_flow!(
    DeleteFlow,
    node_id,
    branch_id,
    workflow_id,
    start_index,
    vertical_index,
    id
);
