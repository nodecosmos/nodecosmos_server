use std::collections::HashSet;

use anyhow::Context;
use charybdis::batch::ModelBatch;
use charybdis::operations::Find;
use charybdis::types::Uuid;

use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::update::BranchUpdate;
use crate::models::branch::Branch;
use crate::models::flow_step::{FlowStep, UpdateInputIdsFlowStep};
use crate::models::io::Io;
use crate::models::traits::{Branchable, FindOrInsertBranched, GroupById, ModelBranchParams};

impl UpdateInputIdsFlowStep {
    pub async fn preserve_branch_ios(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_branch() {
            // for each input and output create branch io if it does not exist
            let io_ids: Vec<Uuid> = self
                .input_ids_by_node_id
                .clone()
                .unwrap_or_default()
                .into_values()
                .flatten()
                .into_iter()
                .collect();

            let ios =
                Io::find_by_branch_id_and_root_id_and_ids(data.db_session(), self.original_id(), self.root_id, &io_ids)
                    .await?
                    .into_iter()
                    .map(|mut io| {
                        io.branch_id = self.branch_id;

                        io
                    })
                    .collect::<Vec<Io>>();

            Io::unlogged_batch()
                .append_inserts_if_not_exist(&ios)
                .execute(data.db_session())
                .await
                .context("Failed to preserve branch ios")?;
        }

        Ok(())
    }

    pub async fn update_ios(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        let current = self.find_by_primary_key().execute(data.db_session()).await?;

        let new_input_ids: HashSet<Uuid> = current
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

        let added_input_ids: HashSet<Uuid> = update_input_ids.difference(&new_input_ids).cloned().collect();
        let removed_input_ids: HashSet<Uuid> = new_input_ids.difference(&update_input_ids).cloned().collect();
        let io_ids: Vec<Uuid> = added_input_ids
            .clone()
            .into_iter()
            .chain(removed_input_ids.clone().into_iter())
            .collect();
        let mut ios_by_id =
            Io::find_by_branch_id_and_root_id_and_ids(data.db_session(), self.branch_id, self.root_id, &io_ids)
                .await?
                .group_by_id()
                .await?;

        if self.is_branch() {
            // add ios that do not exist in the branch
            let ios_to_add =
                Io::find_by_branch_id_and_root_id_and_ids(data.db_session(), self.original_id(), self.root_id, &io_ids)
                    .await?
                    .into_iter()
                    .filter(|io| !ios_by_id.contains_key(&io.id))
                    .map(|mut io| {
                        io.branch_id = self.branch_id;
                        io
                    })
                    .collect();

            Io::unlogged_batch()
                .chunked_insert(data.db_session(), &ios_to_add, 100)
                .await?;

            ios_by_id.extend(ios_to_add.into_iter().map(|io| (io.id, io)));
        }

        let mut batch = Io::statement_batch();

        if added_input_ids.len() > 0 {
            for added_io in added_input_ids {
                batch.append_statement(
                    Io::PUSH_INPUTTED_BY_FLOW_STEPS_IF_EXISTS_QUERY,
                    (vec![self.id], self.branch_id, self.root_id, added_io),
                );
            }
        }

        if removed_input_ids.len() > 0 {
            for removed_io in removed_input_ids {
                batch.append_statement(
                    Io::PULL_INPUTTED_BY_FLOW_STEPS_IF_EXISTS_QUERY,
                    (vec![self.id], self.branch_id, self.root_id, removed_io),
                );
            }
        }

        batch.execute(data.db_session()).await?;

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

        self.preserve_branch_ios(data).await?;

        let [created_ids_by_node_id, removed_ids_by_node_id] =
            FlowStep::ios_diff(current.input_ids_by_node_id, &self.input_ids_by_node_id);

        Branch::update(
            data.db_session(),
            self.branch_id,
            BranchUpdate::CreateFlowStepInputs((self.id, created_ids_by_node_id)),
        )
        .await?;

        Branch::update(
            data.db_session(),
            self.branch_id,
            BranchUpdate::DeleteFlowStepInputs((self.id, removed_ids_by_node_id)),
        )
        .await?;

        Ok(())
    }
}
