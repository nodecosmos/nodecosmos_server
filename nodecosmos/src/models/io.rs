use std::collections::HashSet;

use charybdis::callbacks::Callbacks;
use charybdis::macros::charybdis_model;
use charybdis::operations::Insert;
use charybdis::types::{Boolean, Set, Text, Timestamp, Uuid};
use futures::StreamExt;
use scylla::CachingSession;
use serde::{Deserialize, Serialize};

use macros::{Branchable, Id, MaybeFlowId, MaybeFlowStepId};

use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::archived_io::ArchivedIo;
use crate::models::node::Node;
use crate::models::traits::{Branchable, FindBranchedOrOriginalNode, NodeBranchParams, WhereInChunksExec};
use crate::models::traits::{Context, ModelContext};
use crate::stream::MergedModelStream;

mod create;
mod delete;
mod update_title;

/// Ios are grouped by `root_id`, so they are accessible to all workflows within a same root node.
#[charybdis_model(
    table_name = input_outputs,
    partition_keys = [branch_id],
    clustering_keys = [root_id, id],
    local_secondary_indexes = [main_id, node_id]
)]
#[derive(Branchable, Id, MaybeFlowId, MaybeFlowStepId, Serialize, Deserialize, Default, Clone)]
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

    #[serde(default)]
    pub initial_input: Boolean,

    /// outputted by flow step
    pub flow_step_id: Option<Uuid>,
    pub flow_step_node_id: Option<Uuid>,

    pub inputted_by_flow_steps: Option<Set<Uuid>>,
    pub title: Option<Text>,
    pub unit: Option<Text>,
    pub data_type: Option<Text>,
    pub value: Option<Text>,

    #[serde(default = "chrono::Utc::now")]
    pub created_at: Timestamp,

    #[serde(default = "chrono::Utc::now")]
    pub updated_at: Timestamp,

    #[charybdis(ignore)]
    #[serde(skip)]
    pub node: Option<Node>,

    #[charybdis(ignore)]
    #[serde(skip)]
    pub ctx: Context,

    #[charybdis(ignore)]
    #[serde(skip)]
    pub delete_dangling: Option<Boolean>,
}

impl Callbacks for Io {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_insert(&mut self, db_session: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_default_context() {
            self.validate_attributes(db_session).await?;
            self.copy_vals_from_main(db_session).await?;
        }

        self.preserve_branch_node(data).await?;
        self.preserve_branch_flow_step(data).await?;
        self.update_branch_with_creation(data).await?;

        Ok(())
    }

    async fn after_insert(&mut self, _db_session: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        self.push_to_initial_input_ids(data).await?;
        self.push_to_flow_step_outputs(data).await?;

        Ok(())
    }

    async fn before_delete(&mut self, _: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        self.preserve_branch_flow_step(data).await?;
        self.pull_from_initial_input_ids(data).await?;
        self.pull_from_flow_step_outputs(data).await?;
        self.pull_from_flow_steps_inputs(data).await?;
        self.update_branch_with_deletion(data).await?;

        Ok(())
    }

    async fn after_delete(&mut self, db_session: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        self.create_branched_if_original_exists(data).await?;
        if !self.delete_dangling.map_or(false, |v| v) && self.is_main() && self.flow_step_id.is_some() {
            // NOTE: not the best way to handle this, but we still want to run `before_delete` logic for all ios, but
            // keep the main io in the database as it can be later used by flow steps where it was deleted.
            let mut self_clone = self.clone();
            self_clone.flow_step_id = None;
            self_clone.flow_step_node_id = None;
            self_clone.insert().execute(db_session).await?;
        }

        let _ = ArchivedIo::from(&*self)
            .insert()
            .execute(db_session)
            .await
            .map_err(|e| {
                log::error!("[after_delete] Failed to insert archived io: {:?}", e);
                e
            });

        Ok(())
    }
}

impl Io {
    pub async fn branched(db_session: &CachingSession, params: &NodeBranchParams) -> Result<Vec<Io>, NodecosmosError> {
        // root_id == params.original_id
        let mut ios = Self::find_by_branch_id_and_root_id(params.branch_id, params.root_id)
            .execute(db_session)
            .await?;

        if params.is_original() {
            Ok(ios.try_collect().await?)
        } else {
            let mut original_ios = Self::find_by_branch_id_and_root_id(params.original_id(), params.root_id)
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

    pub async fn find_by_branch_id_and_root_id_and_ids(
        db_session: &CachingSession,
        branch_id: Uuid,
        root_id: Uuid,
        ids: &Set<Uuid>,
    ) -> Result<Vec<Io>, NodecosmosError> {
        let ios = ids
            .where_in_chunked_query(db_session, |ids_chunk| {
                find_io!(
                    "branch_id = ? AND root_id = ? AND id IN ?",
                    (branch_id, root_id, ids_chunk)
                )
            })
            .await
            .try_collect()
            .await?;

        Ok(ios)
    }

    pub async fn find_by_branch_id_and_node_ids(
        db_session: &CachingSession,
        branch_id: Uuid,
        node_ids: &Set<Uuid>,
    ) -> MergedModelStream<Io> {
        node_ids
            .where_in_chunked_query(db_session, |ids_chunk| {
                find_io!("branch_id = ? AND node_id IN ? ALLOW FILTERING", (branch_id, ids_chunk))
            })
            .await
    }

    pub async fn find_or_insert_branched_main(
        data: &RequestData,
        original_id: Uuid,
        branch_id: Uuid,
        main_id: Uuid,
    ) -> Result<Io, NodecosmosError> {
        let io = Self::maybe_find_first_by_branch_id_and_main_id(branch_id, main_id)
            .execute(data.db_session())
            .await?;

        if let Some(io) = io {
            Ok(io)
        } else {
            let mut io = Self::find_first_by_branch_id_and_main_id(original_id, main_id)
                .execute(data.db_session())
                .await?;
            io.branch_id = branch_id;
            io.insert().execute(data.db_session()).await?;
            Ok(io)
        }
    }

    pub async fn find_branched_or_original(
        db_session: &CachingSession,
        root_id: Uuid,
        branch_id: Uuid,
        id: Uuid,
    ) -> Result<Io, NodecosmosError> {
        let is_original = branch_id == root_id;

        if is_original {
            let io = Self::find_first_by_branch_id_and_root_id_and_id(branch_id, root_id, id)
                .execute(db_session)
                .await?;

            Ok(io)
        } else {
            let branched = Self::maybe_find_first_by_branch_id_and_root_id_and_id(branch_id, root_id, id)
                .execute(db_session)
                .await?;

            if let Some(io) = branched {
                Ok(io)
            } else {
                let mut io = Self::find_first_by_branch_id_and_root_id_and_id(root_id, root_id, id)
                    .execute(db_session)
                    .await?;

                io.branch_id = branch_id;

                Ok(io)
            }
        }
    }

    pub fn is_main(&self) -> bool {
        self.main_id == Some(self.id)
    }

    pub async fn node(&mut self, db_session: &CachingSession) -> Result<&mut Node, NodecosmosError> {
        if self.node.is_none() {
            let node = Node::find_branched_or_original(
                db_session,
                NodeBranchParams {
                    root_id: self.root_id,
                    branch_id: self.branch_id,
                    node_id: self.node_id,
                },
            )
            .await?;
            self.node = Some(node);
        }

        Ok(self.node.as_mut().expect("Node should be initialized"))
    }

    /// Main `Io` refers to Io from which we copy values so users don't have to redefine complete IO.
    /// Io description for main and copied remains the same, while properties can be different.
    pub async fn main_io(&self, db_session: &CachingSession) -> Result<Option<Self>, NodecosmosError> {
        if self.is_original() {
            self.original_main_io(db_session).await
        } else {
            self.branched_main_io(db_session).await
        }
    }

    async fn original_main_io(&self, db_session: &CachingSession) -> Result<Option<Self>, NodecosmosError> {
        if let Some(main_id) = self.main_id {
            let res = Self::maybe_find_first_by_branch_id_and_root_id_and_id(self.original_id(), self.root_id, main_id)
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
            let res = Self::maybe_find_first_by_branch_id_and_root_id_and_id(self.branch_id, self.root_id, main_id)
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
        branch_id: Uuid,
        main_id: Uuid,
    ) -> Result<Vec<Self>, NodecosmosError> {
        let ios = UpdateTitleIo::find_by_branch_id_and_main_id(branch_id, main_id)
            .execute(db_session)
            .await?
            .try_collect()
            .await?;

        Ok(ios)
    }

    pub async fn find_by_branch_id_and_root_id_and_ids(
        db_session: &CachingSession,
        branch_id: Uuid,
        root_id: Uuid,
        ids: &Set<Uuid>,
    ) -> Result<Vec<Self>, NodecosmosError> {
        let ios = ids
            .where_in_chunked_query(db_session, |ids_chunk| {
                find_update_title_io!(
                    "branch_id = ? AND root_id = ? AND id IN ?",
                    (branch_id, root_id, ids_chunk)
                )
            })
            .await
            .try_collect()
            .await?;

        Ok(ios)
    }
}

partial_io!(PkIo, branch_id, root_id, id);

impl PkIo {
    pub async fn find_by_branch_id_and_root_id_and_ids(
        db_session: &CachingSession,
        branch_id: Uuid,
        root_id: Uuid,
        ids: &Vec<Uuid>,
    ) -> Result<Vec<Self>, NodecosmosError> {
        let ios = ids
            .where_in_chunked_query(db_session, |ids_chunk| {
                find_pk_io!(
                    "branch_id = ? AND root_id = ? AND id IN ?",
                    (branch_id, root_id, ids_chunk)
                )
            })
            .await
            .try_collect()
            .await?;

        Ok(ios)
    }
}

partial_io!(DeleteIo, root_id, node_id, branch_id, id, flow_id, flow_step_id);

partial_io!(BaseIo, branch_id, node_id, root_id, id);

partial_io!(TitleIo, root_id, node_id, branch_id, id, title, created_at);
