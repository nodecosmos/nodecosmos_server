use crate::errors::NodecosmosError;
use crate::models::flow_step::FlowStep;
use crate::models::workflow::Workflow;
use charybdis::types::Uuid;
use futures::TryStreamExt;
use scylla::CachingSession;
use std::cell::RefCell;
use std::collections::HashMap;

#[derive(Clone)]
pub struct WorkflowDiagram {
    pub flow_step_by_id: HashMap<Uuid, RefCell<FlowStep>>,
    pub flow_steps_ids_by_wf_index: HashMap<u32, Vec<Uuid>>,
    pub flow_step_index_by_id: HashMap<Uuid, u32>,
}

impl WorkflowDiagram {
    pub async fn build(session: &CachingSession, workflow: &Workflow) -> Result<WorkflowDiagram, NodecosmosError> {
        let mut flows = workflow.flows(session).await?;
        let mut flow_step_by_id = HashMap::new();
        let mut flow_steps_ids_by_wf_index = HashMap::new();
        let mut flow_step_index_by_id = HashMap::new();

        while let Some(flow) = flows.try_next().await? {
            let mut flow_steps = flow.flow_steps(session).await?;
            let mut workflow_index = flow.start_index as u32;

            while let Some(flow_step) = flow_steps.try_next().await? {
                flow_steps_ids_by_wf_index
                    .entry(workflow_index)
                    .or_insert_with(Vec::new)
                    .push(flow_step.id);

                flow_step_index_by_id.insert(flow_step.id, workflow_index);

                flow_step_by_id.insert(flow_step.id, RefCell::new(flow_step));

                workflow_index += 1;
            }
        }

        Ok(WorkflowDiagram {
            flow_step_by_id,
            flow_steps_ids_by_wf_index,
            flow_step_index_by_id,
        })
    }

    pub fn flow_steps_by_wf_index(&mut self, index: u32) -> Result<Option<Vec<&RefCell<FlowStep>>>, NodecosmosError> {
        match self.flow_steps_ids_by_wf_index.get(&index) {
            Some(flow_step_ids) => {
                let mut flow_steps = Vec::with_capacity(flow_step_ids.len());

                for flow_step_id in flow_step_ids {
                    let flow_step = self
                        .flow_step_by_id
                        .get(flow_step_id)
                        .ok_or(NodecosmosError::InternalServerError("MissingFlowStep".to_string()))?;

                    flow_steps.push(flow_step);
                }
                Ok(Some(flow_steps))
            }
            _ => Ok(None),
        }
    }

    pub fn flow_step_index(&self, flow_step_id: Uuid) -> Option<u32> {
        self.flow_step_index_by_id.get(&flow_step_id).cloned()
    }
}
