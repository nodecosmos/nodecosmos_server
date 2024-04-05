use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::merge::BranchMerge;
use crate::models::branch::Branch;
use crate::models::flow::Flow;
use crate::models::flow_step::FlowStep;
use crate::models::input_output::Io;
use crate::models::node::PkNode;
use crate::models::traits::{Branchable, ChainOptRef, PluckFromStream, RefCloned};
use crate::models::traits::{FindForBranchMerge, MaybePluckFlowId, MaybePluckFlowStepId, Pluck, PluckFlowId};
use crate::models::udts::Conflict;
use charybdis::types::{Set, Uuid};
use std::collections::HashSet;

pub struct MergeConflicts<'a> {
    branch_merge: &'a mut BranchMerge,
}

impl<'a> MergeConflicts<'a> {
    pub fn new(branch_merge: &'a mut BranchMerge) -> Self {
        Self { branch_merge }
    }

    async fn extract_deleted_ancestors(
        data: &RequestData,
        branch: &mut Branch,
        ancestor_ids: HashSet<Uuid>,
    ) -> Result<(), NodecosmosError> {
        let created_node_ids = branch.created_nodes.ref_cloned();
        let restored_node_ids = branch.restored_nodes.ref_cloned();
        let deleted_node_ids = branch.deleted_nodes.ref_cloned();

        let original_ancestor_ids = ancestor_ids
            .iter()
            .filter_map(|id| {
                if created_node_ids.contains(id) || restored_node_ids.contains(id) || deleted_node_ids.contains(id) {
                    None
                } else {
                    Some(*id)
                }
            })
            .collect::<Vec<Uuid>>();

        let original_ancestor_ids_set = PkNode::find_by_ids(data.db_session(), &original_ancestor_ids)
            .await?
            .pluck_id_set();

        let conflict_deleted_ancestor_ids = original_ancestor_ids
            .iter()
            .filter_map(|id| {
                if original_ancestor_ids_set.contains(id) {
                    None
                } else {
                    Some(*id)
                }
            })
            .collect::<Set<Uuid>>();

        if conflict_deleted_ancestor_ids.is_empty() {
            if let Some(conflict) = &mut branch.conflict {
                conflict.deleted_ancestors = None;
            }

            return Ok(());
        }

        let conflict = branch.conflict.get_or_insert_with(|| Conflict::default());

        conflict.deleted_ancestors = match &conflict.deleted_ancestors {
            Some(deleted_ancestors) => {
                let mut deleted_ancestors = deleted_ancestors.clone();
                deleted_ancestors.extend(conflict_deleted_ancestor_ids);

                Some(deleted_ancestors)
            }
            None => Some(conflict_deleted_ancestor_ids),
        };

        Ok(())
    }

    pub async fn run_check(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        self.extract_created_nodes_conflicts(data).await?;
        self.extract_deleted_edited_nodes(data).await?;
        self.extract_deleted_edited_flows(data).await?;
        self.extract_deleted_edited_flow_steps(data).await?;
        self.extract_diverged_flows(data).await?;
        self.extract_deleted_ios(data).await?;

        Ok(())
    }

    async fn extract_created_nodes_conflicts(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if let Some(created_nodes) = &self.branch_merge.nodes.created_nodes {
            for created_node in created_nodes {
                Self::extract_deleted_ancestors(
                    data,
                    &mut self.branch_merge.branch,
                    created_node.ancestor_ids.ref_cloned(),
                )
                .await?;
            }
        }

        Ok(())
    }

    async fn extract_deleted_edited_nodes(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let created_node_ids = self.branch_merge.branch.created_nodes.ref_cloned();
        let restored_node_ids = self.branch_merge.branch.restored_nodes.ref_cloned();
        let deleted_node_ids = self.branch_merge.branch.deleted_nodes.ref_cloned();
        let edited_description_nodes = self.branch_merge.branch.edited_description_nodes.ref_cloned();
        let mut edited_node_ids = self.branch_merge.branch.edited_workflow_nodes.ref_cloned();

        edited_node_ids.extend(edited_description_nodes);

        let mut deleted_edited_nodes = HashSet::new();

        let original_edited_node_ids = edited_node_ids
            .iter()
            .filter_map(|id| {
                if created_node_ids.contains(id) || deleted_node_ids.contains(id) || restored_node_ids.contains(id) {
                    None
                } else {
                    Some(*id)
                }
            })
            .collect::<Vec<Uuid>>();

        // check ancestors of edited description nodes
        let desc_branched_nodes = PkNode::find_by_ids_and_branch_id(
            data.db_session(),
            &original_edited_node_ids,
            self.branch_merge.branch.id,
        )
        .await?;

        for node in &desc_branched_nodes {
            Self::extract_deleted_ancestors(data, &mut self.branch_merge.branch, node.ancestor_ids.ref_cloned())
                .await?;
        }

        let original_nodes_ids = PkNode::find_by_ids(data.db_session(), &original_edited_node_ids)
            .await?
            .pluck_id_set();

        let deleted_edited_desc_nodes = original_edited_node_ids
            .iter()
            .filter_map(|id| {
                if original_nodes_ids.contains(id) {
                    None
                } else {
                    Some(*id)
                }
            })
            .collect::<Set<Uuid>>();

        deleted_edited_nodes.extend(deleted_edited_desc_nodes);

        if deleted_edited_nodes.len() > 0 {
            self.branch_merge
                .branch
                .conflict
                .get_or_insert_with(|| Conflict::default())
                .deleted_edited_nodes = Some(deleted_edited_nodes);
        } else if let Some(conflict) = &mut self.branch_merge.branch.conflict {
            conflict.deleted_edited_nodes = None;
        }

        Ok(())
    }

    async fn extract_deleted_edited_flows(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let created_flow_ids = self.branch_merge.branch.created_flows.ref_cloned();
        let restored_flow_ids = self.branch_merge.branch.restored_flows.ref_cloned();
        let deleted_flow_ids = self.branch_merge.branch.deleted_flows.ref_cloned();
        let mut edited_flow_ids = self.branch_merge.branch.edited_description_flows.ref_cloned();
        edited_flow_ids.extend(self.branch_merge.flow_steps.created_fs_nodes_flow_steps.pluck_flow_id());
        edited_flow_ids.extend(
            self.branch_merge
                .flow_steps
                .created_fs_inputs_flow_steps
                .pluck_flow_id(),
        );
        edited_flow_ids.extend(
            self.branch_merge
                .flow_steps
                .created_fs_outputs_flow_steps
                .pluck_flow_id(),
        );
        edited_flow_ids.extend(self.branch_merge.ios.created_ios.maybe_pluck_flow_id());

        let original_edited_flow_ids = edited_flow_ids
            .iter()
            .filter_map(|id| {
                if created_flow_ids.contains(id) || deleted_flow_ids.contains(id) || restored_flow_ids.contains(id) {
                    None
                } else {
                    Some(*id)
                }
            })
            .collect::<Set<Uuid>>();

        if let Some(edited_workflow_node_ids) = &self.branch_merge.branch.edited_workflow_nodes {
            let original_flow_ids_set =
                Flow::find_original_by_ids(data.db_session(), edited_workflow_node_ids, &original_edited_flow_ids)
                    .await?
                    .pluck_id_set()
                    .await?;

            let conflict_deleted_edited_flows = original_edited_flow_ids
                .iter()
                .filter_map(|id| {
                    if original_flow_ids_set.contains(id) {
                        None
                    } else {
                        Some(*id)
                    }
                })
                .collect::<Set<Uuid>>();

            if conflict_deleted_edited_flows.len() > 0 {
                self.branch_merge
                    .branch
                    .conflict
                    .get_or_insert_with(|| Conflict::default())
                    .deleted_edited_flows = Some(conflict_deleted_edited_flows);
            } else if let Some(conflict) = &mut self.branch_merge.branch.conflict {
                conflict.deleted_edited_flows = None;
            }
        }

        Ok(())
    }

    async fn extract_deleted_edited_flow_steps(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let created_flow_step_ids = self.branch_merge.branch.created_flow_steps.ref_cloned();
        let restored_flow_step_ids = self.branch_merge.branch.restored_flow_steps.ref_cloned();
        let deleted_flow_step_ids = self.branch_merge.branch.deleted_flow_steps.ref_cloned();
        let mut edited_flow_step_ids = self.branch_merge.branch.edited_description_flow_steps.ref_cloned();
        edited_flow_step_ids.extend(self.branch_merge.flow_steps.created_fs_nodes_flow_steps.pluck_id_set());
        edited_flow_step_ids.extend(self.branch_merge.flow_steps.created_fs_inputs_flow_steps.pluck_id_set());
        edited_flow_step_ids.extend(
            self.branch_merge
                .flow_steps
                .created_fs_outputs_flow_steps
                .pluck_id_set(),
        );
        edited_flow_step_ids.extend(self.branch_merge.ios.created_ios.maybe_pluck_flow_step_id());

        let original_edited_flow_step_ids = edited_flow_step_ids
            .iter()
            .filter_map(|id| {
                if created_flow_step_ids.contains(id)
                    || deleted_flow_step_ids.contains(id)
                    || restored_flow_step_ids.contains(id)
                {
                    None
                } else {
                    Some(*id)
                }
            })
            .collect::<Set<Uuid>>();

        if let Some(edited_workflow_node_ids) = &self.branch_merge.branch.edited_workflow_nodes {
            let original_flow_step_ids_set = FlowStep::find_original_by_ids(
                data.db_session(),
                edited_workflow_node_ids,
                &original_edited_flow_step_ids,
            )
            .await?
            .pluck_id_set()
            .await?;

            let conflict_deleted_edited_flow_steps = original_edited_flow_step_ids
                .iter()
                .filter_map(|id| {
                    if original_flow_step_ids_set.contains(id) {
                        None
                    } else {
                        Some(*id)
                    }
                })
                .collect::<Set<Uuid>>();

            if conflict_deleted_edited_flow_steps.len() > 0 {
                self.branch_merge
                    .branch
                    .conflict
                    .get_or_insert_with(|| Conflict::default())
                    .deleted_edited_flow_steps = Some(conflict_deleted_edited_flow_steps);
            } else if let Some(conflict) = &mut self.branch_merge.branch.conflict {
                conflict.deleted_edited_flow_steps = None;
            }
        }

        Ok(())
    }

    /// Diverged flows are flows that have created_flow_steps whose prev_flow_step's next_flow_step is not the same as
    /// created_flow_step's next_flow_step_id on original flow. User is then given option to choose which flow to keep.
    async fn extract_diverged_flows(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let created_flow_step_ids = self.branch_merge.branch.created_flow_steps.ref_cloned();

        let accepted_flow_solutions = self
            .branch_merge
            .branch
            .accepted_flow_solutions
            .as_ref()
            .cloned()
            .unwrap_or_default();
        let mut diverged_flows = HashSet::new();
        let branch_flow_steps = self
            .branch_merge
            .flow_steps
            .created_flow_steps
            .chain_opt_ref(&self.branch_merge.flow_steps.restored_flow_steps);

        if let Some(branch_flow_steps) = branch_flow_steps {
            for branch_flow_step in branch_flow_steps {
                if accepted_flow_solutions.contains_key(&branch_flow_step.flow_id)
                    || diverged_flows.contains(&branch_flow_step.flow_id)
                {
                    continue;
                }

                let prev_flow_step_id = branch_flow_step.prev_flow_step_id;
                let next_flow_step_id = branch_flow_step.next_flow_step_id;

                match (prev_flow_step_id, next_flow_step_id) {
                    (Some(prev_flow_step_id), Some(next_flow_step_id)) => {
                        let prev_flow_step_next_flow_step_id = if created_flow_step_ids.contains(&prev_flow_step_id) {
                            Some(next_flow_step_id)
                        } else {
                            let prev_flow_step = FlowStep::maybe_find_first_by_node_id_and_branch_id_and_id(
                                branch_flow_step.node_id,
                                branch_flow_step.original_id(),
                                prev_flow_step_id,
                            )
                            .execute(data.db_session())
                            .await?;

                            prev_flow_step.map_or(None, |fs| fs.next_flow_step_id)
                        };

                        let next_flow_step_prev_flow_step_id = if created_flow_step_ids.contains(&next_flow_step_id) {
                            Some(prev_flow_step_id)
                        } else {
                            let next_flow_step = FlowStep::maybe_find_first_by_node_id_and_branch_id_and_id(
                                branch_flow_step.node_id,
                                branch_flow_step.original_id(),
                                next_flow_step_id,
                            )
                            .execute(data.db_session())
                            .await?;

                            next_flow_step.map_or(None, |fs| fs.prev_flow_step_id)
                        };

                        if prev_flow_step_next_flow_step_id != Some(next_flow_step_id)
                            || next_flow_step_prev_flow_step_id != Some(prev_flow_step_id)
                        {
                            diverged_flows.insert(branch_flow_step.flow_id);
                        }
                    }
                    (Some(prev_flow_step_id), None) => {
                        let prev_flow_step_next_flow_step_id = if created_flow_step_ids.contains(&prev_flow_step_id) {
                            None
                        } else {
                            let prev_flow_step = FlowStep::maybe_find_first_by_node_id_and_branch_id_and_id(
                                branch_flow_step.node_id,
                                branch_flow_step.original_id(),
                                prev_flow_step_id,
                            )
                            .execute(data.db_session())
                            .await?;

                            prev_flow_step.map_or(None, |fs| fs.next_flow_step_id)
                        };

                        if prev_flow_step_next_flow_step_id.is_some() {
                            diverged_flows.insert(branch_flow_step.flow_id);
                        }
                    }
                    (None, Some(next_flow_step_id)) => {
                        let next_flow_step_prev_flow_step_id = if created_flow_step_ids.contains(&next_flow_step_id) {
                            None
                        } else {
                            let next_flow_step = FlowStep::maybe_find_first_by_node_id_and_branch_id_and_id(
                                branch_flow_step.node_id,
                                branch_flow_step.original_id(),
                                next_flow_step_id,
                            )
                            .execute(data.db_session())
                            .await?;

                            next_flow_step.map_or(None, |fs| fs.prev_flow_step_id)
                        };

                        if next_flow_step_prev_flow_step_id.is_some() {
                            diverged_flows.insert(branch_flow_step.flow_id);
                        }
                    }
                    (None, None) => {
                        let maybe_first = FlowStep::maybe_find_first_by_node_id_and_branch_id_and_flow_id(
                            branch_flow_step.node_id,
                            branch_flow_step.original_id(),
                            branch_flow_step.flow_id,
                        )
                        .execute(data.db_session())
                        .await?;

                        if maybe_first.is_some() {
                            diverged_flows.insert(branch_flow_step.flow_id);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    async fn extract_deleted_ios(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let created_io_ids = self.branch_merge.branch.created_ios.ref_cloned();
        let restored_io_ids = self.branch_merge.branch.restored_ios.ref_cloned();
        let deleted_io_ids = self.branch_merge.branch.deleted_ios.ref_cloned();
        let edited_io_ids = self.branch_merge.branch.edited_description_ios.ref_cloned();

        let original_edited_io_ids = edited_io_ids
            .iter()
            .filter_map(|id| {
                if created_io_ids.contains(id) || deleted_io_ids.contains(id) || restored_io_ids.contains(id) {
                    None
                } else {
                    Some(*id)
                }
            })
            .collect::<Set<Uuid>>();

        let original_io_ids_set = Io::find_by_root_id_and_branch_id_and_ids(
            data.db_session(),
            self.branch_merge.branch.root_id,
            self.branch_merge.branch.root_id,
            &original_edited_io_ids,
        )
        .await?
        .pluck_id_set();

        let conflict_deleted_edited_ios = original_edited_io_ids
            .iter()
            .filter_map(|id| {
                if original_io_ids_set.contains(id) {
                    None
                } else {
                    Some(*id)
                }
            })
            .collect::<Set<Uuid>>();

        if conflict_deleted_edited_ios.len() > 0 {
            self.branch_merge
                .branch
                .conflict
                .get_or_insert_with(|| Conflict::default())
                .deleted_edited_ios = Some(conflict_deleted_edited_ios);
        } else if let Some(conflict) = &mut self.branch_merge.branch.conflict {
            conflict.deleted_edited_ios = None;
        }

        Ok(())
    }
}
