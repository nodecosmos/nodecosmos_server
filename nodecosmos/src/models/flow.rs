mod callbacks;

use crate::api::WorkflowParams;
use crate::errors::NodecosmosError;
use crate::models::flow_step::FlowStep;
use crate::models::traits::Branchable;
use charybdis::macros::charybdis_model;
use charybdis::stream::CharybdisModelStream;
use charybdis::types::{Double, Int, Text, Timestamp, Uuid};
use futures::StreamExt;
use nodecosmos_macros::BranchableNodeId;
use scylla::CachingSession;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[charybdis_model(
    table_name = flows,
    partition_keys = [node_id],
    clustering_keys = [branch_id, workflow_id, vertical_index, start_index, id],
)]
#[derive(BranchableNodeId, Serialize, Deserialize, Default, Debug)]
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
}

impl Flow {
    pub async fn flow_steps(
        &self,
        session: &CachingSession,
    ) -> Result<CharybdisModelStream<FlowStep>, NodecosmosError> {
        let res = FlowStep::find_by_flow(session, self.node_id, self.branch_id, self.workflow_id, self.id).await?;

        Ok(res)
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
            let mut flow_map: HashMap<Uuid, BaseFlow> = HashMap::new();

            while let Some(flow) = flows.next().await {
                let flow = flow?;
                match flow_map.get(&flow.id) {
                    Some(existing_flow) => {
                        if existing_flow.branch_id == existing_flow.node_id {
                            flow_map.insert(flow.id, flow);
                        }
                    }
                    None => {
                        flow_map.insert(flow.id, flow);
                    }
                }
            }

            Ok(flow_map.into_values().collect())
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
