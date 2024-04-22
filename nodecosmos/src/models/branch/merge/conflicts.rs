use std::collections::HashSet;

use anyhow::Context;
use charybdis::operations::Update;
use charybdis::types::{Set, Uuid};

use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::merge::BranchMerge;
use crate::models::branch::Branch;
use crate::models::flow::Flow;
use crate::models::flow_step::FlowStep;
use crate::models::io::Io;
use crate::models::node::PkNode;
use crate::models::traits::{
    Branchable, ChainOptRef, FlowId, MaybePluckFlowId, PluckFlowId, PluckFromStream, RefCloned,
};
use crate::models::traits::{FindForBranchMerge, MaybePluckFlowStepId, Pluck};
use crate::models::udts::Conflict;

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
                if original_ancestor_ids_set.contains(id)
                    || restored_node_ids.contains(id)
                    || deleted_node_ids.contains(id)
                {
                    None
                } else {
                    Some(*id)
                }
            })
            .collect::<Set<Uuid>>();

        if conflict_deleted_ancestor_ids.len() > 0 {
            branch
                .conflict
                .get_or_insert_with(|| Conflict::default())
                .deleted_ancestors
                .get_or_insert_with(|| Set::new())
                .extend(conflict_deleted_ancestor_ids);
        }

        Ok(())
    }

    pub async fn run_check(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        self.reset_conflicts();
        self.extract_created_nodes_conflicts(data).await?;
        self.extract_deleted_edited_nodes(data).await?;
        self.extract_deleted_edited_flows(data).await?;
        self.extract_deleted_edited_flow_steps(data).await?;
        self.extract_conflicting_flow_steps(data).await?;
        self.extract_deleted_ios(data).await?;

        let branch = &mut self.branch_merge.branch;

        if branch.conflict.as_mut().is_some() {
            branch
                .update()
                .execute(data.db_session())
                .await
                .context("Failed to update branch with resolve")?;
            return Err(NodecosmosError::Conflict("Conflict detected".to_string()));
        }

        Ok(())
    }

    fn reset_conflicts(&mut self) {
        self.branch_merge.branch.conflict = None;
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

        let deleted_edited_nodes = original_edited_node_ids
            .iter()
            .filter_map(|id| {
                if original_nodes_ids.contains(id) {
                    None
                } else {
                    Some(*id)
                }
            })
            .collect::<Set<Uuid>>();

        if deleted_edited_nodes.len() > 0 {
            self.branch_merge
                .branch
                .conflict
                .get_or_insert_with(|| Conflict::default())
                .deleted_edited_nodes = Some(deleted_edited_nodes);
        }

        Ok(())
    }

    async fn extract_deleted_edited_flows(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let mut edited_flow_ids = self.branch_merge.branch.edited_description_flows.ref_cloned();

        self.branch_merge
            .flow_steps
            .created_flow_steps
            .as_ref()
            .map(|flow_steps| {
                edited_flow_ids.extend(flow_steps.pluck_flow_id());
            });

        self.branch_merge
            .flow_steps
            .restored_flow_steps
            .as_ref()
            .map(|flow_steps| {
                edited_flow_ids.extend(flow_steps.pluck_flow_id());
            });

        self.branch_merge
            .flow_steps
            .branched_created_fs_nodes_flow_steps
            .as_ref()
            .map(|flow_steps| {
                edited_flow_ids.extend(flow_steps.values().map(|item| item.flow_id()));
            });

        self.branch_merge
            .flow_steps
            .branched_created_fs_outputs_flow_steps
            .as_ref()
            .map(|flow_steps| {
                edited_flow_ids.extend(flow_steps.values().map(|item| item.flow_id()));
            });

        self.branch_merge
            .flow_steps
            .branched_created_fs_inputs_flow_steps
            .as_ref()
            .map(|flow_steps| {
                edited_flow_ids.extend(flow_steps.values().map(|item| item.flow_id()));
            });

        self.branch_merge.ios.created_ios.as_ref().map(|ios| {
            edited_flow_ids.extend(ios.maybe_pluck_flow_id());
        });

        self.branch_merge.ios.restored_ios.as_ref().map(|ios| {
            edited_flow_ids.extend(ios.maybe_pluck_flow_id());
        });

        let original_edited_flow_ids = edited_flow_ids
            .iter()
            .filter_map(|id| {
                if self
                    .branch_merge
                    .branch
                    .created_flows
                    .as_ref()
                    .is_some_and(|ids| ids.contains(id))
                    || self
                        .branch_merge
                        .branch
                        .deleted_flows
                        .as_ref()
                        .is_some_and(|ids| ids.contains(id))
                    || self
                        .branch_merge
                        .branch
                        .restored_flows
                        .as_ref()
                        .is_some_and(|ids| ids.contains(id))
                {
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
            }
        }

        Ok(())
    }

    async fn extract_deleted_edited_flow_steps(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let mut edited_flow_step_ids = self.branch_merge.branch.edited_description_flow_steps.ref_cloned();

        self.branch_merge
            .flow_steps
            .branched_created_fs_nodes_flow_steps
            .as_ref()
            .map(|flow_steps| {
                edited_flow_step_ids.extend(flow_steps.values().map(|item| item.id));
            });

        self.branch_merge
            .flow_steps
            .branched_created_fs_outputs_flow_steps
            .as_ref()
            .map(|flow_steps| {
                edited_flow_step_ids.extend(flow_steps.values().map(|item| item.id));
            });

        edited_flow_step_ids.extend(self.branch_merge.flow_steps.created_fs_nodes_flow_steps.pluck_id_set());
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
                if self
                    .branch_merge
                    .branch
                    .created_flow_steps
                    .as_ref()
                    .is_some_and(|ids| ids.contains(id))
                    || self
                        .branch_merge
                        .branch
                        .deleted_flow_steps
                        .as_ref()
                        .is_some_and(|ids| ids.contains(id))
                    || self
                        .branch_merge
                        .branch
                        .restored_flow_steps
                        .as_ref()
                        .is_some_and(|ids| ids.contains(id))
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
            }
        }

        Ok(())
    }

    /// Check if flow steps are diverged - if we flow steps within the same flow with the same step_index
    async fn extract_conflicting_flow_steps(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let created_flow_steps = self
            .branch_merge
            .flow_steps
            .created_flow_steps
            .chain_opt_ref(&self.branch_merge.flow_steps.restored_flow_steps);

        if let Some(created_flow_steps) = created_flow_steps {
            let mut conflicting_flow_steps = HashSet::new();

            for flow_step in created_flow_steps {
                if self
                    .branch_merge
                    .branch
                    .created_flows
                    .as_ref()
                    .is_some_and(|ids| ids.contains(&flow_step.flow_id))
                    || self
                        .branch_merge
                        .branch
                        .kept_flow_steps
                        .as_ref()
                        .is_some_and(|ids| ids.contains(&flow_step.id))
                {
                    continue;
                }

                // check if original flow step with same index exists
                let mut maybe_orig = flow_step.clone();
                maybe_orig.set_original_id();

                if let Some(original) = maybe_orig.maybe_find_by_index(data.db_session()).await? {
                    if self
                        .branch_merge
                        .branch
                        .deleted_flow_steps
                        .as_ref()
                        .is_some_and(|ids| ids.contains(&original.id))
                    {
                        continue;
                    }
                    conflicting_flow_steps.insert(flow_step.id);
                }
            }

            if conflicting_flow_steps.len() > 0 {
                self.branch_merge
                    .branch
                    .conflict
                    .get_or_insert_with(|| Conflict::default())
                    .conflicting_flow_steps = Some(conflicting_flow_steps);
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
        }

        Ok(())
    }
}
