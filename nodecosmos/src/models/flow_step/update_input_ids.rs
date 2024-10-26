use std::collections::HashSet;

use charybdis::batch::ModelBatch;
use charybdis::operations::Find;
use charybdis::types::Uuid;

use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::update::BranchUpdate;
use crate::models::branch::Branch;
use crate::models::flow_step::{FlowStep, UpdateInputIdsFlowStep};
use crate::models::io::{Io, PkIo};
use crate::models::traits::{Branchable, FindOriginalOrBranched, Merge, ModelBranchParams};

impl UpdateInputIdsFlowStep {
    pub async fn update_branch(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        Branch::update(data.db_session(), self.branch_id, BranchUpdate::EditNode(self.node_id)).await?;

        let current = FlowStep::find_original_or_branched(
            data.db_session(),
            ModelBranchParams {
                original_id: self.original_id(),
                branch_id: self.branch_id,
                id: self.id,
            },
        )
        .await?;

        let [created_ids_by_node_id, removed_ids_by_node_id] =
            FlowStep::ios_diff(current.input_ids_by_node_id.clone(), &self.input_ids_by_node_id);

        Branch::update(
            data.db_session(),
            self.branch_id,
            BranchUpdate::CreateFlowStepInputs((self.id, created_ids_by_node_id)),
        )
        .await?;

        Branch::update(
            data.db_session(),
            self.branch_id,
            BranchUpdate::DeleteFlowStepInputs((self.id, removed_ids_by_node_id.clone())),
        )
        .await?;

        // TODO: see nodecosmos/src/models/node/create.rs:258
        if current.is_original() {
            current.save_original_data_to_branch(data, self.branch_id).await?;

            self.input_ids_by_node_id.merge_unique(current.input_ids_by_node_id);
        }

        Ok(())
    }

    pub async fn update_ios(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let current = self.find_by_primary_key().execute(data.db_session()).await?;

        let current_input_ids: HashSet<Uuid> = current
            .input_ids_by_node_id
            .clone()
            .unwrap_or_default()
            .into_values()
            .flatten()
            .collect();

        let update_input_ids: HashSet<Uuid> = self
            .input_ids_by_node_id
            .clone()
            .unwrap_or_default()
            .into_values()
            .flatten()
            .collect();

        let added_input_ids: Vec<Uuid> = update_input_ids.difference(&current_input_ids).cloned().collect();
        let removed_input_ids: HashSet<Uuid> = current_input_ids.difference(&update_input_ids).cloned().collect();

        let mut batch = Io::statement_batch();

        if added_input_ids.len() > 0 {
            // scylla 6.0 does not support LWT so we need to map existing ios instead of using if exists
            let existing_io_ids = PkIo::find_by_branch_id_and_root_id_and_ids(
                data.db_session(),
                self.branch_id,
                self.root_id,
                &added_input_ids,
            )
            .await?
            .into_iter()
            .map(|io| io.id)
            .collect::<Vec<Uuid>>();

            for added_io_id in existing_io_ids {
                batch.append_statement(
                    Io::PUSH_INPUTTED_BY_FLOW_STEPS_QUERY,
                    (vec![self.id], self.branch_id, self.root_id, added_io_id),
                );
            }
        }

        if removed_input_ids.len() > 0 {
            for removed_io in removed_input_ids {
                batch.append_statement(
                    Io::PULL_INPUTTED_BY_FLOW_STEPS_QUERY,
                    (vec![self.id], self.branch_id, self.root_id, removed_io),
                );
            }
        }

        batch.execute(data.db_session()).await?;

        Ok(())
    }
}
