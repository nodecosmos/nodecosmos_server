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
use crate::models::io::{Io, PkIo};
use crate::models::traits::{Branchable, FindOrInsertBranched, ModelBranchParams};

impl UpdateInputIdsFlowStep {
    pub async fn preserve_branch_ios(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_branch() {
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

            let branched_ios_ids =
                PkIo::find_by_branch_id_and_root_id_and_ids(data.db_session(), self.branch_id, self.root_id, &io_ids)
                    .await?
                    .into_iter()
                    .map(|io| io.id)
                    .collect::<HashSet<Uuid>>();

            // 6.0 does not support LWT so we use this way to filter out branched ios
            let ios = ios
                .into_iter()
                .filter(|io| !branched_ios_ids.contains(&io.id))
                .collect::<Vec<Io>>();

            Io::unlogged_batch()
                .append_inserts(&ios)
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

        let added_input_ids: Vec<Uuid> = update_input_ids.difference(&new_input_ids).cloned().collect();
        let removed_input_ids: HashSet<Uuid> = new_input_ids.difference(&update_input_ids).cloned().collect();
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
