use crate::errors::NodecosmosError;
use crate::models::flow_step::FlowStep;
use crate::models::helpers::{created_at_cb_fn, impl_updated_at_cb, sanitize_description_cb, updated_at_cb_fn};
use crate::models::workflow::Workflow;
use charybdis::callbacks::Callbacks;
use charybdis::errors::CharybdisError;
use charybdis::macros::charybdis_model;
use charybdis::operations::{Delete, New};
use charybdis::types::{Int, List, Text, Timestamp, Uuid};
use chrono::Utc;
use scylla::CachingSession;
use serde::{Deserialize, Serialize};

#[charybdis_model(
    table_name = flows,
    partition_keys = [node_id],
    clustering_keys = [workflow_id, id],
    global_secondary_indexes = []
)]
#[derive(Serialize, Deserialize, Default)]
pub struct Flow {
    #[serde(rename = "nodeId")]
    pub node_id: Uuid,

    #[serde(rename = "workflowId")]
    pub workflow_id: Uuid,

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

    #[serde(rename = "stepIds")]
    pub step_ids: Option<List<Uuid>>,

    #[serde(rename = "startIndex")]
    pub start_index: Option<Int>,
}

impl Flow {
    fn workflow(&self) -> Workflow {
        let mut workflow = Workflow::new();

        workflow.node_id = self.node_id;
        workflow.id = self.workflow_id;

        workflow
    }

    pub async fn append_step(&mut self, session: &CachingSession, step_id: Uuid) -> Result<(), CharybdisError> {
        self.push_to_step_ids(session, &vec![step_id]).await?;

        Ok(())
    }

    pub async fn remove_step(&mut self, session: &CachingSession, step_id: Uuid) -> Result<(), CharybdisError> {
        self.pull_from_step_ids(session, &vec![step_id]).await?;

        Ok(())
    }
}

impl Callbacks<NodecosmosError> for Flow {
    created_at_cb_fn!();

    async fn after_insert(&mut self, session: &CachingSession) -> Result<(), NodecosmosError> {
        let now = Utc::now();
        self.created_at = Some(now);
        self.updated_at = Some(now);

        self.workflow().append_flow_id(session, self.id).await?;

        Ok(())
    }

    updated_at_cb_fn!();

    async fn after_delete(&mut self, session: &CachingSession) -> Result<(), NodecosmosError> {
        self.workflow().pull_flow_id(session, self.id).await?;

        if let Some(step_ids) = self.step_ids.as_ref() {
            for step_id in step_ids {
                let mut step = FlowStep::new();

                step.node_id = self.node_id;
                step.workflow_id = self.workflow_id;
                step.id = *step_id;

                step.delete(session).await?; // avoid callbacks that are removing steps from the flow
            }
        }

        Ok(())
    }
}

partial_flow!(
    BaseFlow,
    node_id,
    workflow_id,
    id,
    title,
    step_ids,
    start_index,
    created_at,
    updated_at
);

partial_flow!(UpdateFlowTitle, node_id, workflow_id, id, title, updated_at);
impl_updated_at_cb!(UpdateFlowTitle);

partial_flow!(
    FlowDescription,
    node_id,
    workflow_id,
    id,
    description,
    description_markdown,
    updated_at
);
sanitize_description_cb!(FlowDescription);

partial_flow!(FlowDelete, node_id, workflow_id, id);
