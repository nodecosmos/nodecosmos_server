use std::collections::HashSet;

use charybdis::callbacks::Callbacks;
use charybdis::macros::charybdis_model;
use charybdis::operations::{DeleteWithCallbacks, Insert};
use charybdis::types::{Double, Int, Text, Timestamp, Uuid};
use futures::StreamExt;
use scylla::client::caching_session::CachingSession;
use serde::{Deserialize, Serialize};

use macros::{Branchable, Id};

use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::archived_flow::ArchivedFlow;
use crate::models::flow_step::FlowStep;
use crate::models::traits::{Branchable, NodeBranchParams};
use crate::models::traits::{Context, ModelContext};

pub mod create;
mod update_title;

#[charybdis_model(
    table_name = flows,
    partition_keys = [branch_id],
    clustering_keys = [node_id, vertical_index, start_index, id],
    local_secondary_indexes = [id]
)]
#[derive(Id, Branchable, Serialize, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Flow {
    pub node_id: Uuid,
    pub branch_id: Uuid,

    #[branch(original_id)]
    pub root_id: Uuid,

    // vertical index
    pub vertical_index: Double,

    // start index is not conflicting, flows can start at same index
    pub start_index: Int,

    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,

    #[serde(default)]
    pub title: Text,

    #[serde(default = "chrono::Utc::now")]
    pub created_at: Timestamp,

    #[serde(default = "chrono::Utc::now")]
    pub updated_at: Timestamp,

    #[charybdis(ignore)]
    #[serde(skip)]
    pub ctx: Context,
}

impl Callbacks for Flow {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_insert(&mut self, _db_session: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_default_context() {
            self.id = Uuid::new_v4();

            self.update_branch_with_creation(data).await?;
        }

        if self.is_default_context() || self.is_branch_init_context() {
            self.preserve_branch_node(data).await?;
        }

        if !self.is_branch_init_context() {
            self.calculate_vertical_idx(data).await?;
        }

        Ok(())
    }

    async fn before_delete(&mut self, db_session: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        self.update_branch_with_deletion(data).await?;
        self.preserve_branch_node(data).await?;

        if self.is_original() {
            let flow_steps = self.flow_steps(db_session).await?;

            for mut flow_step in flow_steps {
                flow_step.set_parent_delete_context();
                flow_step.delete_cb(data).execute(db_session).await?;
            }
        }

        Ok(())
    }

    async fn after_delete(&mut self, _db_session: &CachingSession, data: &RequestData) -> Result<(), Self::Error> {
        // TODO: see nodecosmos/src/models/node/create.rs:258
        self.create_branched_if_original_exists(data).await?;

        let _ = ArchivedFlow::from(&*self)
            .insert()
            .execute(data.db_session())
            .await
            .map_err(|e| {
                log::error!("[after_delete] Failed to insert archived flow: {:?}", e);
                e
            });

        Ok(())
    }
}

impl Flow {
    /// merges original and branched flows
    pub async fn branched(
        db_session: &CachingSession,
        params: &NodeBranchParams,
    ) -> Result<Vec<Self>, NodecosmosError> {
        let mut flows = Self::find_by_branch_id_and_node_id(params.branch_id, params.node_id)
            .execute(db_session)
            .await?;

        if params.is_original() {
            Ok(flows.try_collect().await?)
        } else {
            let mut original_flows = Self::find_by_branch_id_and_node_id(params.original_id(), params.node_id)
                .execute(db_session)
                .await?;
            let mut branched_flows_set = HashSet::new();
            let mut branch_flows = vec![];

            while let Some(flow) = flows.next().await {
                let flow = flow?;
                branched_flows_set.insert(flow.id);
                branch_flows.push(flow);
            }

            while let Some(flow) = original_flows.next().await {
                let mut flow = flow?;
                if !branched_flows_set.contains(&flow.id) {
                    flow.branch_id = params.branch_id;
                    branch_flows.push(flow);
                }
            }

            branch_flows.sort_by(|a, b| {
                a.vertical_index
                    .partial_cmp(&b.vertical_index)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            Ok(branch_flows)
        }
    }

    pub async fn flow_steps(&self, db_session: &CachingSession) -> Result<Vec<FlowStep>, NodecosmosError> {
        let res = FlowStep::find_by_flow(
            db_session,
            &NodeBranchParams {
                root_id: self.root_id,
                node_id: self.node_id,
                branch_id: self.branch_id,
            },
            self.id,
        )
        .await?;

        Ok(res)
    }
}

partial_flow!(
    UpdateTitleFlow,
    node_id,
    branch_id,
    start_index,
    vertical_index,
    id,
    root_id,
    title,
    updated_at,
    ctx
);

impl Callbacks for UpdateTitleFlow {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_update(&mut self, _db_session: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        self.update_branch(data).await?;

        Ok(())
    }
}

partial_flow!(PkFlow, node_id, branch_id, start_index, vertical_index, id, root_id);

partial_flow!(
    TitleFlow,
    node_id,
    branch_id,
    start_index,
    vertical_index,
    id,
    root_id,
    title,
    created_at
);
