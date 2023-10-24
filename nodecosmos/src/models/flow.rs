use crate::errors::NodecosmosError;
use crate::models::flow_step::FlowStep;
use crate::models::helpers::{created_at_cb_fn, impl_updated_at_cb, sanitize_description_cb, updated_at_cb_fn};
use charybdis::callbacks::Callbacks;
use charybdis::macros::charybdis_model;
use charybdis::stream::CharybdisModelStream;
use charybdis::types::{Double, Int, Text, Timestamp, Uuid};
use scylla::CachingSession;
use serde::{Deserialize, Serialize};

#[charybdis_model(
    table_name = flows,
    partition_keys = [node_id],
    clustering_keys = [workflow_id, start_index, vertical_index, id],
)]
#[derive(Serialize, Deserialize, Default)]
pub struct Flow {
    #[serde(rename = "nodeId")]
    pub node_id: Uuid,

    #[serde(rename = "workflowId")]
    pub workflow_id: Uuid,

    // start index is not conflicting, flows can start at same index
    #[serde(rename = "startIndex")]
    pub start_index: Int,

    // vertical index
    #[serde(rename = "verticalIndex")]
    pub vertical_index: Double,

    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,

    pub title: Option<Text>,
    pub description: Option<Text>,

    #[serde(rename = "descriptionMarkdown")]
    pub description_markdown: Option<Text>,

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
        let res =
            FlowStep::find_by_node_id_and_workflow_id_and_flow_id(session, self.node_id, self.workflow_id, self.id)
                .await?;

        Ok(res)
    }
}

impl Callbacks<NodecosmosError> for Flow {
    created_at_cb_fn!();

    updated_at_cb_fn!();

    async fn after_delete(&self, session: &CachingSession) -> Result<(), NodecosmosError> {
        FlowStep::delete_by_node_id_and_workflow_id_and_flow_id(session, self.node_id, self.workflow_id, self.id)
            .await?;

        Ok(())
    }
}

partial_flow!(
    BaseFlow,
    node_id,
    workflow_id,
    start_index,
    vertical_index,
    id,
    title,
    created_at,
    updated_at
);

partial_flow!(
    UpdateFlowTitle,
    node_id,
    workflow_id,
    start_index,
    vertical_index,
    id,
    title,
    updated_at
);
impl_updated_at_cb!(UpdateFlowTitle);

partial_flow!(
    FlowDescription,
    node_id,
    workflow_id,
    start_index,
    vertical_index,
    id,
    description,
    description_markdown,
    updated_at
);
sanitize_description_cb!(FlowDescription);

partial_flow!(DeleteFlow, node_id, workflow_id, start_index, vertical_index, id);
