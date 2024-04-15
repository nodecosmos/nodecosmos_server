use std::collections::HashSet;

use charybdis::callbacks::Callbacks;
use charybdis::macros::charybdis_model;
use charybdis::operations::Insert;
use charybdis::types::{Frozen, List, Set, Text, Timestamp, Uuid};
use futures::StreamExt;
use scylla::CachingSession;
use serde::{Deserialize, Serialize};

use nodecosmos_macros::{Branchable, Id, MaybeFlowId, MaybeFlowStepId};

use crate::api::data::RequestData;
use crate::api::WorkflowParams;
use crate::errors::NodecosmosError;
use crate::models::flow_step::FlowStep;
use crate::models::node::Node;
use crate::models::traits::{Branchable, FindBranchedOrOriginal};
use crate::models::traits::{Context, ModelContext};
use crate::models::udts::Property;

mod create;
mod delete;
mod update_title;

/// Ios are grouped by `root_id`, so they are accessible to all workflows within a same root node.
/// Original Ios are the ones where `branch_id` == `root_id`.
#[charybdis_model(
    table_name = input_outputs,
    partition_keys = [root_id, branch_id],
    clustering_keys = [id],
    local_secondary_indexes = [main_id]
)]
#[derive(Branchable, Id, MaybeFlowId, MaybeFlowStepId, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Io {
    #[branch(original_id)]
    pub root_id: Uuid,
    pub node_id: Uuid,
    pub branch_id: Uuid,

    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,

    pub main_id: Option<Uuid>,
    pub flow_id: Option<Uuid>,

    /// outputted by flow step
    pub flow_step_id: Option<Uuid>,

    pub inputted_by_flow_steps: Option<Set<Uuid>>,
    pub title: Option<Text>,
    pub unit: Option<Text>,
    pub data_type: Option<Text>,
    pub value: Option<Text>,
    pub properties: Option<Frozen<List<Frozen<Property>>>>,

    #[serde(default = "chrono::Utc::now")]
    pub created_at: Timestamp,

    #[serde(default = "chrono::Utc::now")]
    pub updated_at: Timestamp,

    #[charybdis(ignore)]
    #[serde(skip)]
    pub flow_step: Option<FlowStep>,

    #[charybdis(ignore)]
    #[serde(skip)]
    pub node: Option<Node>,

    #[charybdis(ignore)]
    #[serde(skip)]
    pub ctx: Context,
}

impl Callbacks for Io {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_insert(&mut self, db_session: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_default_context() {
            self.validate_root_id(db_session).await?;
            self.set_defaults();
            self.copy_vals_from_main(db_session).await?;
            self.update_branch_with_creation(data).await?;
        }

        if self.is_default_context() || self.is_branched_init_context() {
            self.preserve_branch_node(data).await?;
            self.preserve_branch_flow_step(data).await?;
        }

        Ok(())
    }

    async fn before_delete(&mut self, db_session: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_default_context() {
            self.pull_from_initial_input_ids(db_session).await?;
            self.pull_form_flow_step_outputs(data).await?;
            self.preserve_branch_flow_step(data).await?;
        }

        if !self.is_merge_context() {
            self.pull_from_flow_steps_inputs(data).await?;
            self.update_branch_with_deletion(data).await?;
        }

        Ok(())
    }

    async fn after_delete(&mut self, _db_session: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        self.create_branched_if_original_exists(data).await?;

        Ok(())
    }
}

impl Io {
    pub async fn branched(
        db_session: &CachingSession,
        root_id: Uuid,
        params: &WorkflowParams,
    ) -> Result<Vec<Io>, NodecosmosError> {
        let mut ios = Self::find_by_root_id_and_branch_id(root_id, params.branch_id)
            .execute(db_session)
            .await?;

        if params.is_original() {
            Ok(ios.try_collect().await?)
        } else {
            let mut original_ios = Self::find_by_root_id_and_branch_id(root_id, root_id)
                .execute(db_session)
                .await?;
            let mut branched_ios_set = HashSet::new();
            let mut branch_ios = vec![];

            while let Some(io) = ios.next().await {
                let io = io?;
                branched_ios_set.insert(io.id);
                branch_ios.push(io);
            }

            while let Some(io) = original_ios.next().await {
                let mut io = io?;
                if !branched_ios_set.contains(&io.id) {
                    io.branch_id = params.branch_id;
                    branch_ios.push(io);
                }
            }

            Ok(branch_ios)
        }
    }

    pub async fn find_by_root_id_and_branch_id_and_ids(
        db_session: &CachingSession,
        root_id: Uuid,
        branch_id: Uuid,
        ids: &Set<Uuid>,
    ) -> Result<Vec<Io>, NodecosmosError> {
        let ios = find_io!("root_id = ? AND branch_id = ? AND id IN ?", (root_id, branch_id, ids))
            .execute(db_session)
            .await?
            .try_collect()
            .await?;

        Ok(ios)
    }

    pub async fn node(&mut self, db_session: &CachingSession) -> Result<&mut Node, NodecosmosError> {
        if self.node.is_none() {
            let node = Node::find_branched_or_original(db_session, self.node_id, self.branch_id).await?;
            self.node = Some(node);
        }

        Ok(self.node.as_mut().expect("Node should be initialized"))
    }

    pub async fn flow_step(&mut self, db_session: &CachingSession) -> Result<&mut Option<FlowStep>, NodecosmosError> {
        if let Some(flow_step_id) = self.flow_step_id {
            if self.flow_step.is_none() {
                let flow_step =
                    FlowStep::find_first_by_node_id_and_branch_id_and_id(self.node_id, self.branch_id, flow_step_id)
                        .execute(db_session)
                        .await?;
                self.flow_step = Some(flow_step);
            }
        }

        Ok(&mut self.flow_step)
    }

    /// Main `Io` refers to Io from which we copy values so users don't have to redefine complete IO.
    /// Io description for main and copied remains the same, while properties can be different.
    pub async fn main_io(&self, db_session: &CachingSession) -> Result<Option<Self>, NodecosmosError> {
        return if self.is_original() {
            self.original_main_io(db_session).await
        } else {
            self.branched_main_io(db_session).await
        };
    }

    async fn original_main_io(&self, db_session: &CachingSession) -> Result<Option<Self>, NodecosmosError> {
        if let Some(main_id) = self.main_id {
            let res = Self::maybe_find_first_by_root_id_and_branch_id_and_id(self.root_id, self.original_id(), main_id)
                .execute(db_session)
                .await?;

            Ok(res)
        } else {
            Ok(None)
        }
    }

    pub async fn branched_main_io(&self, db_session: &CachingSession) -> Result<Option<Self>, NodecosmosError> {
        if self.main_id.is_some() {
            if let Some(branched) = self.maybe_main(db_session).await? {
                return Ok(Some(branched));
            }

            if let Some(mut main_io) = self.original_main_io(db_session).await? {
                main_io.branch_id = self.branch_id;

                main_io.insert().execute(db_session).await?;

                return Ok(Some(main_io));
            }

            return Ok(None);
        }

        Ok(None)
    }

    async fn maybe_main(&self, db_session: &CachingSession) -> Result<Option<Self>, NodecosmosError> {
        if let Some(main_id) = self.main_id {
            let res = Self::maybe_find_first_by_root_id_and_branch_id_and_id(self.root_id, self.branch_id, main_id)
                .execute(db_session)
                .await?;

            return Ok(res);
        }

        Ok(None)
    }
}

partial_io!(
    UpdateTitleIo,
    root_id,
    node_id,
    branch_id,
    id,
    main_id,
    title,
    updated_at,
    ctx
);

impl Callbacks for UpdateTitleIo {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_update(&mut self, db_session: &CachingSession, data: &RequestData) -> Result<(), Self::Error> {
        self.update_branch(data).await?;
        self.update_ios_titles_by_main_id(db_session).await?;

        Ok(())
    }
}

impl UpdateTitleIo {
    pub async fn ios_by_main_id(
        db_session: &CachingSession,
        root_id: Uuid,
        branch_id: Uuid,
        main_id: Uuid,
    ) -> Result<Vec<Self>, NodecosmosError> {
        let ios = UpdateTitleIo::find_by_root_id_and_branch_id_and_main_id(root_id, branch_id, main_id)
            .execute(db_session)
            .await?
            .try_collect()
            .await?;

        Ok(ios)
    }

    pub async fn find_by_root_id_and_branch_id_and_ids(
        db_session: &CachingSession,
        root_id: Uuid,
        branch_id: Uuid,
        ids: &Set<Uuid>,
    ) -> Result<Vec<Self>, NodecosmosError> {
        let ios = find_update_title_io!("root_id = ? AND branch_id = ? AND id IN ?", (root_id, branch_id, ids))
            .execute(db_session)
            .await?
            .try_collect()
            .await?;

        Ok(ios)
    }
}

partial_io!(DeleteIo, root_id, node_id, branch_id, id, flow_id, flow_step_id);
