use charybdis::batch::ModelBatch;
use charybdis::types::{Set, Uuid};
use futures::StreamExt;
use scylla::client::caching_session::CachingSession;
use serde::{Deserialize, Serialize};

use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::archived_description::{ArchivedDescription, PkArchivedDescription};
use crate::models::archived_flow::{ArchivedFlow, PkArchivedFlow};
use crate::models::archived_flow_step::{ArchivedFlowStep, PkArchivedFlowStep};
use crate::models::archived_io::{ArchivedIo, PkArchivedIo};
use crate::models::archived_node::{ArchivedNode, PkArchiveNode};
use crate::models::archived_workflow::{ArchivedWorkflow, PkArchivedWorkflow};
use crate::models::attachment::{Attachment, AttachmentsDelete};
use crate::models::branch::update::BranchUpdate;
use crate::models::branch::Branch;
use crate::models::description::Description;
use crate::models::flow::Flow;
use crate::models::flow_step::FlowStep;
use crate::models::io::Io;
use crate::models::like::Like;
use crate::models::node::Node;
use crate::models::node_descendant::NodeDescendant;
use crate::models::recovery::{RecoveryLog, RecoveryObjectType};
use crate::models::traits::{Branchable, ElasticDocument, ModelContext, Pluck};
use crate::models::traits::{Descendants, FindForBranchMerge};
use crate::models::workflow::Workflow;

impl Node {
    pub async fn archive_and_delete(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_original() {
            let mut node_delete = NodeDelete::new(data, self).await?;
            node_delete.run(data).await?;

            // if we are in merge context, we need to store the delete data for potential recovery
            if self.is_merge_context() {
                self.delete_data = Some(Box::new(node_delete));
            }
        } else if Branch::contains_created_node(data.db_session(), self.branch_id, self.id).await?
            || Self::is_original_deleted(data.db_session(), self.original_id(), self.id).await?
        {
            NodeDelete::new(data, self).await?.run(data).await?;
        }

        Ok(())
    }

    pub async fn update_branch_with_deletion(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_branch() {
            let mut node_ids = vec![self.id];
            let descendant_ids = self
                .descendants(data.db_session())
                .await?
                .try_collect()
                .await?
                .pluck_id();
            node_ids.extend(descendant_ids);

            Branch::update(data.db_session(), self.branch_id, BranchUpdate::DeleteNodes(node_ids)).await?;
        }

        Ok(())
    }
}

// we don't delete counter data as counter row can not be recreated
#[derive(Clone, Copy, Default, Serialize, Deserialize, PartialOrd, PartialEq, Debug)]
pub enum NodeDeleteStep {
    BeforeStart = -1,
    #[default]
    Start = 0,
    ArchiveNodes = 1,
    ArchiveWorkflows = 2,
    ArchiveFlows = 3,
    ArchiveFlowSteps = 4,
    ArchiveIos = 5,
    ArchiveDescriptions = 6,
    DeleteNodes = 7,
    DeleteDescendants = 8,
    DeleteWorkflows = 9,
    DeleteFlows = 10,
    DeleteFlowSteps = 11,
    DeleteIos = 12,
    DeleteDescriptions = 13,
    DeleteLikes = 14,
    DeleteAttachments = 15,
    DeleteElasticData = 16,
    Finish = 17,
    AfterFinish = 18,
}

impl NodeDeleteStep {
    pub fn increment(&mut self) {
        *self = NodeDeleteStep::from(*self as i8 + 1);
    }

    pub fn decrement(&mut self) {
        *self = NodeDeleteStep::from(*self as i8 - 1);
    }
}

impl From<i8> for NodeDeleteStep {
    fn from(value: i8) -> Self {
        match value {
            -1 => NodeDeleteStep::BeforeStart,
            0 => NodeDeleteStep::Start,
            1 => NodeDeleteStep::ArchiveNodes,
            2 => NodeDeleteStep::ArchiveWorkflows,
            3 => NodeDeleteStep::ArchiveFlows,
            4 => NodeDeleteStep::ArchiveFlowSteps,
            5 => NodeDeleteStep::ArchiveIos,
            6 => NodeDeleteStep::ArchiveDescriptions,
            7 => NodeDeleteStep::DeleteNodes,
            8 => NodeDeleteStep::DeleteDescendants,
            9 => NodeDeleteStep::DeleteWorkflows,
            10 => NodeDeleteStep::DeleteFlows,
            11 => NodeDeleteStep::DeleteFlowSteps,
            12 => NodeDeleteStep::DeleteIos,
            13 => NodeDeleteStep::DeleteDescriptions,
            14 => NodeDeleteStep::DeleteLikes,
            15 => NodeDeleteStep::DeleteAttachments,
            16 => NodeDeleteStep::DeleteElasticData,
            17 => NodeDeleteStep::Finish,
            18 => NodeDeleteStep::AfterFinish,
            _ => panic!("Invalid NodeDeleteStep value: {}", value),
        }
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct NodeDelete {
    node: Node,
    delete_step: NodeDeleteStep,
    deleted_node_ids: Vec<Uuid>,
    deleted_nodes: Vec<Node>,
    deleted_descendants: Vec<NodeDescendant>,
    deleted_workflows: Vec<Workflow>,
    deleted_flows: Vec<Flow>,
    deleted_flow_steps: Vec<FlowStep>,
    deleted_ios: Vec<Io>,
    deleted_descriptions: Vec<Description>,
}

impl NodeDelete {
    async fn deleted_nodes(
        db_session: &CachingSession,
        node: &Node,
        ids: &Set<Uuid>,
    ) -> Result<Vec<Node>, NodecosmosError> {
        Node::find_by_ids(db_session, node.branch_id, &ids.iter().cloned().collect())
            .await
            .try_collect()
            .await}

    pub async fn deleted_descendants(
        db_session: &CachingSession,
        node: &Node,
        ids: &Set<Uuid>,
    ) -> Result<Vec<NodeDescendant>, NodecosmosError> {
        let mut descendants;

        if let Some(ancestor_ids) = &node.ancestor_ids {
            let query_ids = ids.clone().into_iter().chain(ancestor_ids.clone()).collect();

            descendants = vec![];

            let mut fetched_desc =
                NodeDescendant::find_by_node_ids(db_session, node.root_id, node.branch_id, &query_ids).await;

            while let Some(desc) = fetched_desc.next().await {
                let desc = desc?;
                if ids.contains(&desc.id) {
                    descendants.push(desc);
                }
            }
        } else {
            descendants = NodeDescendant::find_by_node_ids(
                db_session,
                node.root_id,
                node.branch_id,
                &ids.iter().cloned().collect(),
            )
            .await
            .try_collect()
            .await?;
        }

        Ok(descendants)
    }

    async fn deleted_workflows(
        db_session: &CachingSession,
        node: &Node,
        ids: &Set<Uuid>,
    ) -> Result<Vec<Workflow>, NodecosmosError> {
        Workflow::find_by_node_ids(db_session, node.branch_id, &ids.iter().cloned().collect())
            .await
            .try_collect()
            .await}

    async fn deleted_flows(
        db_session: &CachingSession,
        node: &Node,
        ids: &Set<Uuid>,
    ) -> Result<Vec<Flow>, NodecosmosError> {
        Flow::find_by_branch_id_and_node_ids(db_session, node.branch_id, ids)
            .await
            .try_collect()
            .await}

    async fn deleted_flow_steps(
        db_session: &CachingSession,
        node: &Node,
        ids: &Set<Uuid>,
    ) -> Result<Vec<FlowStep>, NodecosmosError> {
        FlowStep::find_by_branch_id_and_node_ids(db_session, node.branch_id, ids)
            .await
            .try_collect()
            .await}

    async fn deleted_ios(
        db_session: &CachingSession,
        node: &Node,
        ids: &Set<Uuid>,
    ) -> Result<Vec<Io>, NodecosmosError> {
        Io::find_by_branch_id_and_node_ids(db_session, node.branch_id, ids)
            .await
            .try_collect()
            .await}

    async fn deleted_descriptions(
        db_session: &CachingSession,
        branch_id: Uuid,
        ids_to_del: Set<Uuid>,
    ) -> Result<Vec<Description>, NodecosmosError> {
        Description::find_by_branch_id_and_ids(db_session, branch_id, &ids_to_del)
            .await
            .try_collect()
            .await}

    pub async fn new(data: &RequestData, node: &Node) -> Result<NodeDelete, NodecosmosError> {
        let mut node_ids_to_delete = Set::new();
        node_ids_to_delete.insert(node.id);

        let mut descendants = node.descendants(data.db_session()).await?;

        while let Some(descendant) = descendants.next().await {
            let descendant = descendant?;

            node_ids_to_delete.insert(descendant.id);
        }

        let deleted_nodes = Self::deleted_nodes(data.db_session(), node, &node_ids_to_delete).await?;
        let deleted_descendants = Self::deleted_descendants(data.db_session(), node, &node_ids_to_delete).await?;
        let deleted_workflows = Self::deleted_workflows(data.db_session(), node, &node_ids_to_delete).await?;
        let deleted_flows = Self::deleted_flows(data.db_session(), node, &node_ids_to_delete).await?;
        let deleted_flow_steps = Self::deleted_flow_steps(data.db_session(), node, &node_ids_to_delete).await?;
        let deleted_ios = Self::deleted_ios(data.db_session(), node, &node_ids_to_delete).await?;

        let desc_ids = deleted_nodes
            .pluck_id_set()
            .into_iter()
            .chain(deleted_flows.pluck_id_set())
            .chain(deleted_flow_steps.pluck_id_set())
            .chain(deleted_ios.pluck_id_set())
            .collect();

        let deleted_descriptions = Self::deleted_descriptions(data.db_session(), node.branch_id, desc_ids).await?;

        Ok(Self {
            node: node.clone(),
            delete_step: NodeDeleteStep::Start,
            deleted_node_ids: deleted_nodes.iter().map(|node| node.id).collect(),
            deleted_nodes,
            deleted_descendants,
            deleted_workflows,
            deleted_flows,
            deleted_flow_steps,
            deleted_ios,
            deleted_descriptions,
        })
    }

    pub async fn run(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let delete = self.delete(data).await;

        if let Err(e) = delete {
            self.recover(data)
                .await
                .map_err(|e| NodecosmosError::FatalDeleteError(format!("Error deleting node: {:?}", e)))?;

            return Err(e);
        }

        Ok(())
    }

    pub async fn delete(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        while self.delete_step <= NodeDeleteStep::Finish {
            // log current step
            if self.delete_step > NodeDeleteStep::Start && self.delete_step < NodeDeleteStep::Finish {
                self.update_recovery_log_step(data.db_session(), self.delete_step as i8)
                    .await?;
            }

            match self.delete_step {
                NodeDeleteStep::BeforeStart => {
                    log::error!("should not hit before placeholder");
                }
                NodeDeleteStep::Start => {
                    self.create_recovery_log(data.db_session()).await?;
                }
                NodeDeleteStep::ArchiveNodes => self.archive_nodes(data.db_session()).await?,
                NodeDeleteStep::ArchiveWorkflows => self.archive_workflows(data.db_session()).await?,
                NodeDeleteStep::ArchiveFlows => self.archive_flows(data.db_session()).await?,
                NodeDeleteStep::ArchiveFlowSteps => self.archive_flow_steps(data.db_session()).await?,
                NodeDeleteStep::ArchiveIos => self.archive_ios(data.db_session()).await?,
                NodeDeleteStep::ArchiveDescriptions => self.archive_descriptions(data.db_session()).await?,
                NodeDeleteStep::DeleteNodes => self.delete_nodes(data.db_session()).await?,
                NodeDeleteStep::DeleteDescendants => self.delete_descendants(data.db_session()).await?,
                NodeDeleteStep::DeleteWorkflows => self.delete_workflows(data.db_session()).await?,
                NodeDeleteStep::DeleteFlows => self.delete_flows(data.db_session()).await?,
                NodeDeleteStep::DeleteFlowSteps => self.delete_flow_steps(data.db_session()).await?,
                NodeDeleteStep::DeleteIos => self.delete_ios(data.db_session()).await?,
                NodeDeleteStep::DeleteDescriptions => self.delete_descriptions(data.db_session()).await?,
                NodeDeleteStep::DeleteLikes => self.delete_likes(data.db_session()).await?,
                NodeDeleteStep::DeleteAttachments => self.delete_attachments(data).await?,
                NodeDeleteStep::DeleteElasticData => self.delete_elastic_data(data).await,
                NodeDeleteStep::Finish => {
                    self.delete_recovery_log(data.db_session()).await?;
                }
                NodeDeleteStep::AfterFinish => {
                    log::error!("should not hit after placeholder");
                }
            }

            self.delete_step.increment();
        }

        Ok(())
    }

    pub async fn recover(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        while self.delete_step >= NodeDeleteStep::Start {
            // log current step
            if self.delete_step > NodeDeleteStep::Start && self.delete_step < NodeDeleteStep::Finish {
                self.update_recovery_log_step(data.db_session(), self.delete_step as i8)
                    .await?;
            }

            match self.delete_step {
                NodeDeleteStep::BeforeStart => {
                    log::error!("should not hit before placeholder");
                }
                NodeDeleteStep::Start => {
                    self.delete_recovery_log(data.db_session()).await?;
                }
                NodeDeleteStep::ArchiveNodes => self.undo_archive_nodes(data.db_session()).await?,
                NodeDeleteStep::ArchiveWorkflows => self.undo_archive_workflows(data.db_session()).await?,
                NodeDeleteStep::ArchiveFlows => self.undo_archive_flows(data.db_session()).await?,
                NodeDeleteStep::ArchiveFlowSteps => self.undo_archive_flow_steps(data.db_session()).await?,
                NodeDeleteStep::ArchiveIos => self.undo_archive_ios(data.db_session()).await?,
                NodeDeleteStep::ArchiveDescriptions => self.undo_archive_descriptions(data.db_session()).await?,
                NodeDeleteStep::DeleteNodes => self.undo_delete_nodes(data.db_session()).await?,
                NodeDeleteStep::DeleteDescendants => self.undo_delete_descendants(data.db_session()).await?,
                NodeDeleteStep::DeleteWorkflows => self.undo_delete_workflows(data.db_session()).await?,
                NodeDeleteStep::DeleteFlows => self.undo_delete_flows(data.db_session()).await?,
                NodeDeleteStep::DeleteFlowSteps => self.undo_delete_flow_steps(data.db_session()).await?,
                NodeDeleteStep::DeleteIos => self.undo_delete_ios(data.db_session()).await?,
                NodeDeleteStep::DeleteDescriptions => self.undo_delete_descriptions(data.db_session()).await?,
                NodeDeleteStep::DeleteLikes => self.undo_delete_likes().await?,
                NodeDeleteStep::DeleteAttachments => self.undo_delete_attachments().await?,
                NodeDeleteStep::DeleteElasticData => self.undo_delete_elastic_data(data).await,
                NodeDeleteStep::Finish => {
                    log::error!("should not recover finished process");
                }
                NodeDeleteStep::AfterFinish => {
                    log::error!("should not hit after placeholder");
                }
            }

            self.delete_step.decrement();
        }

        Ok(())
    }

    async fn archive_nodes(&self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        let mut archive_nodes = vec![];

        for node in &self.deleted_nodes {
            archive_nodes.push(ArchivedNode::from(node));
        }

        ArchivedNode::unlogged_batch()
            .chunked_insert(db_session, &archive_nodes, crate::constants::BATCH_CHUNK_SIZE)
            .await
            .map_err(NodecosmosError::from)
    }

    async fn undo_archive_nodes(&self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        let mut archive_nodes = vec![];

        for node in &self.deleted_nodes {
            archive_nodes.push(PkArchiveNode {
                id: node.id,
                branch_id: node.branch_id,
            });
        }

        PkArchiveNode::unlogged_batch()
            .chunked_delete(db_session, &archive_nodes, crate::constants::BATCH_CHUNK_SIZE)
            .await
            .map_err(NodecosmosError::from)
    }

    async fn archive_workflows(&self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        let mut archive_workflows = vec![];

        for workflow in &self.deleted_workflows {
            archive_workflows.push(ArchivedWorkflow::from(workflow));
        }

        ArchivedWorkflow::unlogged_batch()
            .chunked_insert(db_session, &archive_workflows, crate::constants::BATCH_CHUNK_SIZE)
            .await
            .map_err(NodecosmosError::from)
    }

    async fn undo_archive_workflows(&self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        let mut archive_workflows = vec![];

        for workflow in &self.deleted_workflows {
            archive_workflows.push(PkArchivedWorkflow {
                node_id: workflow.node_id,
                branch_id: workflow.branch_id,
            });
        }

        PkArchivedWorkflow::unlogged_batch()
            .chunked_delete(db_session, &archive_workflows, crate::constants::BATCH_CHUNK_SIZE)
            .await
            .map_err(NodecosmosError::from)
    }

    async fn archive_flows(&self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        let mut archive_flows = vec![];

        for flow in &self.deleted_flows {
            archive_flows.push(ArchivedFlow::from(flow));
        }

        ArchivedFlow::unlogged_batch()
            .chunked_insert(db_session, &archive_flows, crate::constants::BATCH_CHUNK_SIZE)
            .await
            .map_err(NodecosmosError::from)
    }

    async fn undo_archive_flows(&self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        let mut archive_flows = vec![];

        for flow in &self.deleted_flows {
            archive_flows.push(PkArchivedFlow::from(flow));
        }

        PkArchivedFlow::unlogged_batch()
            .chunked_delete(db_session, &archive_flows, crate::constants::BATCH_CHUNK_SIZE)
            .await
            .map_err(NodecosmosError::from)
    }

    async fn archive_flow_steps(&self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        let mut archive_flow_steps = vec![];

        for flow_step in &self.deleted_flow_steps {
            archive_flow_steps.push(ArchivedFlowStep::from(flow_step));
        }

        ArchivedFlowStep::unlogged_batch()
            .chunked_insert(db_session, &archive_flow_steps, crate::constants::BATCH_CHUNK_SIZE)
            .await
            .map_err(NodecosmosError::from)
    }

    async fn undo_archive_flow_steps(&self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        let mut archive_flow_steps = vec![];

        for flow_step in &self.deleted_flow_steps {
            archive_flow_steps.push(PkArchivedFlowStep::from(flow_step));
        }

        PkArchivedFlowStep::unlogged_batch()
            .chunked_delete(db_session, &archive_flow_steps, crate::constants::BATCH_CHUNK_SIZE)
            .await
            .map_err(NodecosmosError::from)
    }

    async fn archive_ios(&self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        let mut archive_ios = vec![];

        for io in &self.deleted_ios {
            archive_ios.push(ArchivedIo::from(io));
        }

        ArchivedIo::unlogged_batch()
            .chunked_insert(db_session, &archive_ios, crate::constants::BATCH_CHUNK_SIZE)
            .await
            .map_err(NodecosmosError::from)
    }

    async fn undo_archive_ios(&self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        let mut archive_ios = vec![];

        for io in &self.deleted_ios {
            archive_ios.push(PkArchivedIo::from(io));
        }

        PkArchivedIo::unlogged_batch()
            .chunked_delete(db_session, &archive_ios, crate::constants::BATCH_CHUNK_SIZE)
            .await
            .map_err(NodecosmosError::from)
    }

    async fn archive_descriptions(&self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        let mut archive_descriptions = vec![];

        for description in &self.deleted_descriptions {
            archive_descriptions.push(ArchivedDescription::from(description));
        }

        ArchivedDescription::unlogged_batch()
            .chunked_insert(db_session, &archive_descriptions, crate::constants::BATCH_CHUNK_SIZE)
            .await
            .map_err(NodecosmosError::from)
    }

    async fn undo_archive_descriptions(&self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        let mut archive_descriptions = vec![];

        for description in &self.deleted_descriptions {
            archive_descriptions.push(PkArchivedDescription::from(description));
        }

        PkArchivedDescription::unlogged_batch()
            .chunked_delete(db_session, &archive_descriptions, crate::constants::BATCH_CHUNK_SIZE)
            .await
            .map_err(NodecosmosError::from)
    }

    async fn delete_nodes(&self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        Node::unlogged_batch()
            .chunked_delete(db_session, &self.deleted_nodes, crate::constants::BATCH_CHUNK_SIZE)
            .await
            .map_err(NodecosmosError::from)
    }

    async fn undo_delete_nodes(&self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        Node::unlogged_batch()
            .chunked_insert(db_session, &self.deleted_nodes, crate::constants::BATCH_CHUNK_SIZE)
            .await
            .map_err(NodecosmosError::from)
    }

    async fn delete_descendants(&self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        NodeDescendant::unlogged_batch()
            .chunked_delete(
                db_session,
                &self.deleted_descendants,
                crate::constants::BATCH_CHUNK_SIZE,
            )
            .await
            .map_err(NodecosmosError::from)
    }

    async fn undo_delete_descendants(&self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        NodeDescendant::unlogged_batch()
            .chunked_insert(
                db_session,
                &self.deleted_descendants,
                crate::constants::BATCH_CHUNK_SIZE,
            )
            .await
            .map_err(NodecosmosError::from)
    }

    async fn delete_workflows(&self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        Workflow::unlogged_batch()
            .chunked_delete(db_session, &self.deleted_workflows, crate::constants::BATCH_CHUNK_SIZE)
            .await
            .map_err(NodecosmosError::from)
    }

    async fn undo_delete_workflows(&self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        Workflow::unlogged_batch()
            .chunked_insert(db_session, &self.deleted_workflows, crate::constants::BATCH_CHUNK_SIZE)
            .await
            .map_err(NodecosmosError::from)
    }

    async fn delete_flows(&self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        Flow::unlogged_batch()
            .chunked_delete(db_session, &self.deleted_flows, crate::constants::BATCH_CHUNK_SIZE)
            .await
            .map_err(NodecosmosError::from)
    }

    async fn undo_delete_flows(&self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        Flow::unlogged_batch()
            .chunked_insert(db_session, &self.deleted_flows, crate::constants::BATCH_CHUNK_SIZE)
            .await
            .map_err(NodecosmosError::from)
    }

    async fn delete_flow_steps(&self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        FlowStep::unlogged_batch()
            .chunked_delete(db_session, &self.deleted_flow_steps, crate::constants::BATCH_CHUNK_SIZE)
            .await
            .map_err(NodecosmosError::from)
    }

    async fn undo_delete_flow_steps(&self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        FlowStep::unlogged_batch()
            .chunked_insert(db_session, &self.deleted_flow_steps, crate::constants::BATCH_CHUNK_SIZE)
            .await
            .map_err(NodecosmosError::from)
    }

    async fn delete_ios(&self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        Io::unlogged_batch()
            .chunked_delete(db_session, &self.deleted_ios, crate::constants::BATCH_CHUNK_SIZE)
            .await
            .map_err(NodecosmosError::from)
    }

    async fn undo_delete_ios(&self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        Io::unlogged_batch()
            .chunked_insert(db_session, &self.deleted_ios, crate::constants::BATCH_CHUNK_SIZE)
            .await
            .map_err(NodecosmosError::from)
    }

    async fn delete_descriptions(&self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        Description::unlogged_batch()
            .chunked_delete(
                db_session,
                &self.deleted_descriptions,
                crate::constants::BATCH_CHUNK_SIZE,
            )
            .await
            .map_err(NodecosmosError::from)
    }

    async fn undo_delete_descriptions(&self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        Description::unlogged_batch()
            .chunked_insert(
                db_session,
                &self.deleted_descriptions,
                crate::constants::BATCH_CHUNK_SIZE,
            )
            .await
            .map_err(NodecosmosError::from)
    }

    // like undo is not critical, so we won't load it into memory as in case of merge
    // we might have a lot of likes to restore
    async fn delete_likes(&self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        let mut deleted_likes: Vec<Like> = vec![];

        for id in &self.deleted_node_ids {
            deleted_likes.push(Like {
                object_id: *id,
                branch_id: self.node.branch_id,
                ..Default::default()
            });
        }

        let _ = Like::unlogged_batch()
            .chunked_delete_by_partition_key(db_session, &deleted_likes, crate::constants::BATCH_CHUNK_SIZE)
            .await
            .map_err(|e| log::error!("Error deleting likes: {:?}", e));

        Ok(())
    }

    // not critical for delete/merge
    async fn undo_delete_likes(&self) -> Result<(), NodecosmosError> {
        Ok(())
    }

    async fn delete_elastic_data(&self, data: &RequestData) {
        if self.node.is_original() {
            let _ = Node::bulk_delete_elastic_documents(data.elastic_client(), &self.deleted_node_ids).await;
        }
    }

    async fn undo_delete_elastic_data(&self, data: &RequestData) {
        if self.node.is_original() {
            let _ = Node::bulk_insert_elastic_documents(data.elastic_client(), &self.deleted_nodes).await;
        }
    }

    // not critical for delete/merge
    pub async fn delete_attachments(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        Attachment::find_by_node_ids(data.db_session(), self.node.branch_id, &self.deleted_node_ids)
            .await?
            .delete_all(data)?;

        Ok(())
    }

    // not critical for delete/merge
    pub async fn undo_delete_attachments(&mut self) -> Result<(), NodecosmosError> {
        Ok(())
    }
}

impl RecoveryLog<'_> for NodeDelete {
    fn rec_id(&self) -> Uuid {
        self.node.id
    }

    fn rec_branch_id(&self) -> Uuid {
        self.node.branch_id
    }

    fn rec_object_type(&self) -> RecoveryObjectType {
        RecoveryObjectType::NodeDelete
    }

    fn set_step(&mut self, step: i8) {
        self.delete_step = NodeDeleteStep::from(step);
    }

    async fn recover_from_log(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        self.recover(data).await.map_err(|e| {
            log::error!("FatalDeleteError recovering from log: {:?}", e);
            NodecosmosError::FatalDeleteError(format!("Error recovering from log: {:?}", e))
        })
    }
}
