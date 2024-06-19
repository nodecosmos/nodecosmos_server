use std::collections::{HashMap, HashSet};

use charybdis::operations::DeleteWithCallbacks;

use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::update::BranchUpdate;
use crate::models::branch::Branch;
use crate::models::flow_step::{FlowStep, UpdateNodeIdsFlowStep};
use crate::models::io::Io;
use crate::models::traits::{Branchable, FindOrInsertBranched, ModelBranchParams, ModelContext, ToHashSet};

impl UpdateNodeIdsFlowStep {
    pub async fn delete_output_records_from_removed_nodes(
        &mut self,
        data: &RequestData,
    ) -> Result<(), NodecosmosError> {
        if let Some(output_ids_by_node_id) = self.output_ids_by_node_id.as_ref() {
            for (node_id, output_ids) in output_ids_by_node_id.iter() {
                if self.node_ids.is_none() || self.node_ids.as_ref().is_some_and(|ids| !ids.contains(node_id)) {
                    let outputs = Io::find_by_branch_id_and_root_id_and_ids(
                        data.db_session(),
                        self.branch_id,
                        self.root_id,
                        output_ids,
                    )
                    .await?;

                    for mut output in outputs {
                        output.set_parent_delete_context();
                        output.delete_cb(data).execute(data.db_session()).await?;
                    }
                }
            }
        }

        Ok(())
    }

    pub async fn remove_output_references_from_removed_nodes(&mut self) -> Result<(), NodecosmosError> {
        if let Some(output_ids_by_node_id) = self.output_ids_by_node_id.as_mut() {
            output_ids_by_node_id.retain(|node_id, _| self.node_ids.as_ref().is_some_and(|ids| ids.contains(node_id)));
        }

        Ok(())
    }

    pub async fn remove_input_references_from_removed_nodes(&mut self) -> Result<(), NodecosmosError> {
        if let Some(input_ids_by_node_id) = self.input_ids_by_node_id.as_mut() {
            input_ids_by_node_id.retain(|node_id, _| self.node_ids.as_ref().is_some_and(|ids| ids.contains(node_id)));
        }

        Ok(())
    }

    pub async fn update_branch(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        Branch::update(data.db_session(), self.branch_id, BranchUpdate::EditNode(self.node_id)).await?;

        let current = FlowStep::find_or_insert_branched(
            data,
            ModelBranchParams {
                original_id: self.original_id(),
                branch_id: self.branch_id,
                id: self.id,
            },
        )
        .await?;

        current.preserve_flow_step_outputs(data).await?;

        let current_db_node_ids = current.node_ids.clone();
        let new_node_ids = self.node_ids.clone();

        let mut created_node_ids = HashSet::new();
        let mut deleted_node_ids = HashSet::new();

        match (current_db_node_ids, new_node_ids) {
            (Some(current_db_node_ids), Some(new_node_ids)) => {
                // Calculate created and deleted nodes
                let created: HashSet<_> = new_node_ids
                    .iter()
                    .filter(|id| !current_db_node_ids.contains(id))
                    .cloned()
                    .collect();

                let deleted: HashSet<_> = current_db_node_ids
                    .iter()
                    .filter(|id| !new_node_ids.contains(id))
                    .cloned()
                    .collect();

                if !created.is_empty() {
                    created_node_ids = created;
                }
                if !deleted.is_empty() {
                    deleted_node_ids = deleted;
                }
            }
            (Some(current_db_node_ids), None) => {
                // all current db node ids are considered deleted
                deleted_node_ids = current_db_node_ids.to_hash_set();
            }
            (None, Some(new_node_ids)) => {
                // all new node ids are considered created
                created_node_ids = new_node_ids.to_hash_set();
            }
            (None, None) => {}
        }

        let mut created_flow_step_nodes = HashMap::new();
        created_flow_step_nodes.insert(self.id, created_node_ids);

        let mut deleted_flow_step_nodes = HashMap::new();
        deleted_flow_step_nodes.insert(self.id, deleted_node_ids);

        Branch::update(
            data.db_session(),
            self.branch_id,
            BranchUpdate::CreateFlowStepNodes(created_flow_step_nodes),
        )
        .await?;

        Branch::update(
            data.db_session(),
            self.branch_id,
            BranchUpdate::DeleteFlowStepNodes(deleted_flow_step_nodes),
        )
        .await?;

        Ok(())
    }
}
