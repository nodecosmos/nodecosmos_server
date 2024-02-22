mod callbacks;
mod create;
mod delete;
mod update_description;
mod update_title;

use crate::errors::NodecosmosError;
use crate::models::flow_step::FlowStep;
use crate::models::node::Node;
use crate::models::udts::Property;
use crate::models::workflow::Workflow;
use crate::utils::deserializer::required;
use charybdis::macros::charybdis_model;
use charybdis::types::{Frozen, List, Text, Timestamp, Uuid};
use scylla::CachingSession;
use serde::{Deserialize, Serialize};

/// we group input outputs by root node id
/// so they are accessible to all workflows within a same root node
#[charybdis_model(
    table_name = input_outputs,
    partition_keys = [root_node_id],
    clustering_keys = [node_id, workflow_id, id],
    local_secondary_indexes = [
        ([root_node_id], [id]),
        ([root_node_id], [original_id]),
    ]
)]
#[derive(Serialize, Deserialize, Default)]
pub struct Io {
    #[serde(rename = "rootNodeId")]
    pub root_node_id: Uuid,

    #[serde(rename = "nodeId")]
    pub node_id: Uuid,

    #[serde(rename = "workflowId")]
    pub workflow_id: Uuid,

    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,

    #[serde(rename = "originalId", deserialize_with = "required")]
    pub original_id: Option<Uuid>,

    /// outputted by flow step
    #[serde(rename = "flowStepId")]
    pub flow_step_id: Option<Uuid>,

    pub title: Option<Text>,

    pub unit: Option<Text>,

    #[serde(rename = "dataType")]
    pub data_type: Option<Text>,

    pub value: Option<Text>,

    pub description: Option<Text>,

    #[serde(rename = "descriptionMarkdown")]
    pub description_markdown: Option<Text>,

    #[serde(rename = "descriptionBase64")]
    pub description_base64: Option<Text>,

    pub properties: Option<Frozen<List<Frozen<Property>>>>,

    #[serde(rename = "createdAt")]
    pub created_at: Option<Timestamp>,

    #[serde(rename = "updatedAt")]
    pub updated_at: Option<Timestamp>,

    #[charybdis(ignore)]
    #[serde(skip)]
    pub flow_step: Option<FlowStep>,

    #[charybdis(ignore)]
    #[serde(skip)]
    pub workflow: Option<Workflow>,

    #[charybdis(ignore)]
    #[serde(skip)]
    pub node: Option<Node>,
}

impl Io {
    pub async fn workflow(&mut self, session: &CachingSession) -> Result<&mut Option<Workflow>, NodecosmosError> {
        if self.workflow.is_none() {
            if let Some(flow_step) = &mut self.flow_step {
                let workflow = flow_step.workflow(session).await?;
                self.workflow = workflow.clone();
            } else {
                let workflow = Workflow::by_node_id_and_id(session, self.node_id, self.workflow_id).await?;
                self.workflow = Some(workflow);
            }
        }

        Ok(&mut self.workflow)
    }

    pub async fn node(&mut self, session: &CachingSession) -> Result<&mut Node, NodecosmosError> {
        if self.node.is_none() {
            // TODO: introduce branch_id to workflow
            let node = Node::find_by_id_and_branch_id(self.node_id, self.node_id)
                .execute(session)
                .await?;
            self.node = Some(node);
        }

        Ok(self.node.as_mut().unwrap())
    }

    pub async fn flow_step(&mut self, session: &CachingSession) -> Result<&mut Option<FlowStep>, NodecosmosError> {
        if let Some(flow_step_id) = self.flow_step_id {
            if self.flow_step.is_none() {
                let flow_step = FlowStep::find_by_node_id_and_id(session, self.node_id, flow_step_id).await?;
                self.flow_step = Some(flow_step);
            }
        }

        Ok(&mut self.flow_step)
    }

    pub async fn original_io(&self, session: &CachingSession) -> Result<Option<Self>, NodecosmosError> {
        if let Some(original_id) = self.original_id {
            let original_io = find_first_io!("root_node_id = ? AND id = ?", (self.root_node_id, original_id))
                .execute(session)
                .await?;
            Ok(Some(original_io))
        } else {
            Ok(None)
        }
    }
}

partial_io!(
    GetDescriptionIo,
    root_node_id,
    node_id,
    workflow_id,
    id,
    description,
    description_markdown
);

partial_io!(
    UpdateDescriptionIo,
    root_node_id,
    node_id,
    workflow_id,
    id,
    original_id,
    description,
    description_markdown,
    updated_at
);

impl UpdateDescriptionIo {
    pub async fn ios_by_original_id(
        session: &CachingSession,
        root_node_id: Uuid,
        original_id: Uuid,
    ) -> Result<Vec<Self>, NodecosmosError> {
        let ios = find_update_description_io!("root_node_id = ? AND original_id = ?", (root_node_id, original_id))
            .execute(session)
            .await?
            .try_collect()
            .await?;

        Ok(ios)
    }
}

partial_io!(
    UpdateTitleIo,
    root_node_id,
    node_id,
    workflow_id,
    id,
    original_id,
    title,
    updated_at
);

impl UpdateTitleIo {
    pub async fn ios_by_original_id(
        session: &CachingSession,
        root_node_id: Uuid,
        original_id: Uuid,
    ) -> Result<Vec<Self>, NodecosmosError> {
        let ios = find_update_title_io!("root_node_id = ? AND original_id = ?", (root_node_id, original_id))
            .execute(session)
            .await?
            .try_collect()
            .await?;

        Ok(ios)
    }
}

partial_io!(UpdateWorkflowIndexIo, root_node_id, node_id, workflow_id, id);

partial_io!(DeleteIo, root_node_id, node_id, workflow_id, id, flow_step_id);
