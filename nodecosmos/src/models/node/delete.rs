use charybdis::batch::ModelBatch;
use charybdis::types::{Set, Uuid};
use futures::StreamExt;
use scylla::CachingSession;
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
use crate::models::node_counter::NodeCounter;
use crate::models::node_descendant::NodeDescendant;
use crate::models::traits::{Branchable, ElasticDocument, Pluck};
use crate::models::traits::{Descendants, FindForBranchMerge};
use crate::models::workflow::Workflow;

impl Node {
    pub async fn archive_and_delete(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_original() {
            NodeDelete::new(self, &data).await?.run().await?;
        } else if Branch::contains_created_node(data.db_session(), self.branch_id, self.id).await?
            || Self::is_original_deleted(data.db_session(), self.id).await?
        {
            NodeDelete::new(self, &data).await?.run().await?;
        }

        Ok(())
    }

    pub async fn update_branch_with_deletion(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_branched() {
            let mut node_ids = vec![self.id];
            let descendant_ids = self
                .descendants(data.db_session())
                .await?
                .try_collect()
                .await?
                .pluck_id();
            node_ids.extend(descendant_ids);

            Branch::update(&data, self.branch_id, BranchUpdate::DeleteNodes(node_ids)).await?;
        }

        Ok(())
    }
}

#[derive(Clone, Copy, Default, Serialize, Deserialize, PartialOrd, PartialEq, Debug)]
enum NodeDeleteStep {
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
    DeleteCounterData = 14,
    DeleteLikes = 15,
    DeleteAttachments = 16,
    DeleteElasticData = 17,
    Finish = 18,
}

impl NodeDeleteStep {
    pub fn increment(&mut self) {
        *self = NodeDeleteStep::from(*self as u8 + 1);
    }

    pub fn decrement(&mut self) {
        *self = NodeDeleteStep::from(*self as u8 - 1);
    }
}

impl From<u8> for NodeDeleteStep {
    fn from(value: u8) -> Self {
        match value {
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
            14 => NodeDeleteStep::DeleteCounterData,
            15 => NodeDeleteStep::DeleteLikes,
            16 => NodeDeleteStep::DeleteAttachments,
            17 => NodeDeleteStep::DeleteElasticData,
            18 => NodeDeleteStep::Finish,
            _ => NodeDeleteStep::Start,
        }
    }
}

pub struct NodeDelete<'a> {
    delete_step: NodeDeleteStep,
    data: &'a RequestData,
    node: &'a Node,
    deleted_node_ids: Vec<Uuid>,
    deleted_nodes: Vec<Node>,
    deleted_descendants: Vec<NodeDescendant>,
    deleted_workflows: Vec<Workflow>,
    deleted_flows: Vec<Flow>,
    deleted_flow_steps: Vec<FlowStep>,
    deleted_ios: Vec<Io>,
    deleted_descriptions: Vec<Description>,
    deleted_counter_data: Vec<NodeCounter>,
}

impl<'a> NodeDelete<'a> {
    async fn deleted_nodes(
        db_session: &CachingSession,
        node: &Node,
        ids: &Set<Uuid>,
    ) -> Result<Vec<Node>, NodecosmosError> {
        let nodes = if node.is_branched() {
            Node::find_by_ids_and_branch_id(db_session, ids, node.branch_id).await?
        } else {
            Node::find_by_ids(db_session, ids).await?
        };

        nodes.try_collect().await.map_err(NodecosmosError::from)
    }

    pub async fn deleted_descendants(
        db_session: &CachingSession,
        node: &Node,
        ids: &Set<Uuid>,
    ) -> Result<Vec<NodeDescendant>, NodecosmosError> {
        let descendants = if node.is_branched() {
            NodeDescendant::find_by_branch_id_and_node_ids(db_session, node.root_id, node.branch_id, ids).await?
        } else {
            NodeDescendant::find_by_node_ids(db_session, node.root_id, ids).await?
        };

        descendants.try_collect().await.map_err(NodecosmosError::from)
    }

    async fn deleted_workflows(
        db_session: &CachingSession,
        node: &Node,
        ids: &Set<Uuid>,
    ) -> Result<Vec<Workflow>, NodecosmosError> {
        let workflows = if node.is_branched() {
            Workflow::find_by_node_ids_and_branch_id(db_session, ids, node.branch_id).await?
        } else {
            Workflow::find_by_node_ids(db_session, ids).await?
        };

        workflows.try_collect().await.map_err(NodecosmosError::from)
    }

    async fn deleted_flows(
        db_session: &CachingSession,
        node: &Node,
        ids: &Set<Uuid>,
    ) -> Result<Vec<Flow>, NodecosmosError> {
        let flows = if node.is_branched() {
            Flow::find_by_node_ids_and_branch_id(db_session, ids, node.branch_id).await?
        } else {
            Flow::find_original_by_node_ids(db_session, ids).await?
        };

        flows.try_collect().await.map_err(NodecosmosError::from)
    }

    async fn deleted_flow_steps(
        db_session: &CachingSession,
        node: &Node,
        ids: &Set<Uuid>,
    ) -> Result<Vec<FlowStep>, NodecosmosError> {
        let flow_steps = if node.is_branched() {
            FlowStep::find_by_node_ids_and_branch_id(db_session, ids, node.branch_id).await?
        } else {
            FlowStep::find_original_by_node_ids(db_session, ids).await?
        };

        flow_steps.try_collect().await.map_err(NodecosmosError::from)
    }

    async fn deleted_ios(
        db_session: &CachingSession,
        node: &Node,
        ids: &Set<Uuid>,
    ) -> Result<Vec<Io>, NodecosmosError> {
        let ios = if node.is_branched() {
            Io::find_by_root_id_and_branch_id_and_node_ids(db_session, node.root_id, node.branch_id, ids).await?
        } else {
            Io::find_by_root_id_and_node_ids(db_session, node.root_id, ids).await?
        };

        ios.try_collect().await.map_err(NodecosmosError::from)
    }

    async fn deleted_descriptions(
        db_session: &CachingSession,
        node: &Node,
        ids: &Set<Uuid>,
    ) -> Result<Vec<Description>, NodecosmosError> {
        let descriptions = if node.is_branched() {
            Description::find_by_node_ids_and_branch_id(db_session, ids, node.branch_id).await?
        } else {
            Description::find_original_by_node_ids(db_session, ids).await?
        };

        descriptions.try_collect().await.map_err(NodecosmosError::from)
    }

    async fn deleted_counter_data(
        db_session: &CachingSession,
        node: &Node,
        ids: &Set<Uuid>,
    ) -> Result<Vec<NodeCounter>, NodecosmosError> {
        let counters = if node.is_branched() {
            NodeCounter::find_by_ids_and_branch_id(db_session, ids, node.branch_id).await?
        } else {
            NodeCounter::find_by_ids(db_session, ids).await?
        };

        counters.try_collect().await.map_err(NodecosmosError::from)
    }

    pub async fn new(node: &'a Node, data: &'a RequestData) -> Result<NodeDelete<'a>, NodecosmosError> {
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
        let deleted_descriptions = Self::deleted_descriptions(data.db_session(), node, &node_ids_to_delete).await?;
        let deleted_counter_data = Self::deleted_counter_data(data.db_session(), node, &node_ids_to_delete).await?;

        Ok(Self {
            delete_step: NodeDeleteStep::Start,
            data,
            node,
            deleted_node_ids: deleted_nodes.iter().map(|node| node.id).collect(),
            deleted_nodes,
            deleted_descendants,
            deleted_workflows,
            deleted_flows,
            deleted_flow_steps,
            deleted_ios,
            deleted_descriptions,
            deleted_counter_data,
        })
    }

    pub async fn run(&mut self) -> Result<(), NodecosmosError> {
        let delete = self.delete().await;

        if let Err(e) = delete {
            self.recover()
                .await
                .map_err(|e| NodecosmosError::FatalDeleteError(format!("Error deleting node: {:?}", e)))?;

            return Err(e);
        }

        Ok(())
    }

    pub async fn delete(&mut self) -> Result<(), NodecosmosError> {
        while self.delete_step <= NodeDeleteStep::Finish {
            match self.delete_step {
                NodeDeleteStep::Start => (),
                NodeDeleteStep::ArchiveNodes => self.archive_nodes(self.data.db_session()).await?,
                NodeDeleteStep::ArchiveWorkflows => self.archive_workflows(self.data.db_session()).await?,
                NodeDeleteStep::ArchiveFlows => self.archive_flows(self.data.db_session()).await?,
                NodeDeleteStep::ArchiveFlowSteps => self.archive_flow_steps(self.data.db_session()).await?,
                NodeDeleteStep::ArchiveIos => self.archive_ios(self.data.db_session()).await?,
                NodeDeleteStep::ArchiveDescriptions => self.archive_descriptions(self.data.db_session()).await?,
                NodeDeleteStep::DeleteNodes => self.delete_nodes(self.data.db_session()).await?,
                NodeDeleteStep::DeleteDescendants => self.delete_descendants(self.data.db_session()).await?,
                NodeDeleteStep::DeleteWorkflows => self.delete_workflows(self.data.db_session()).await?,
                NodeDeleteStep::DeleteFlows => self.delete_flows(self.data.db_session()).await?,
                NodeDeleteStep::DeleteFlowSteps => self.delete_flow_steps(self.data.db_session()).await?,
                NodeDeleteStep::DeleteIos => self.delete_ios(self.data.db_session()).await?,
                NodeDeleteStep::DeleteDescriptions => self.delete_descriptions(self.data.db_session()).await?,
                NodeDeleteStep::DeleteCounterData => self.delete_counter_data(self.data.db_session()).await?,
                NodeDeleteStep::DeleteLikes => self.delete_likes(self.data.db_session()).await?,
                NodeDeleteStep::DeleteAttachments => self.delete_attachments().await?,
                NodeDeleteStep::DeleteElasticData => self.delete_elastic_data().await,
                NodeDeleteStep::Finish => (),
            }

            self.delete_step.increment();
        }

        Ok(())
    }

    pub async fn recover(&mut self) -> Result<(), NodecosmosError> {
        while self.delete_step > NodeDeleteStep::Start {
            self.delete_step.decrement();

            match self.delete_step {
                NodeDeleteStep::Start => (),
                NodeDeleteStep::ArchiveNodes => self.undo_archive_nodes(self.data.db_session()).await?,
                NodeDeleteStep::ArchiveWorkflows => self.undo_archive_workflows(self.data.db_session()).await?,
                NodeDeleteStep::ArchiveFlows => self.undo_archive_flows(self.data.db_session()).await?,
                NodeDeleteStep::ArchiveFlowSteps => self.undo_archive_flow_steps(self.data.db_session()).await?,
                NodeDeleteStep::ArchiveIos => self.undo_archive_ios(self.data.db_session()).await?,
                NodeDeleteStep::ArchiveDescriptions => self.undo_archive_descriptions(self.data.db_session()).await?,
                NodeDeleteStep::DeleteNodes => self.undo_delete_nodes(self.data.db_session()).await?,
                NodeDeleteStep::DeleteDescendants => self.undo_delete_descendants(self.data.db_session()).await?,
                NodeDeleteStep::DeleteWorkflows => self.undo_delete_workflows(self.data.db_session()).await?,
                NodeDeleteStep::DeleteFlows => self.undo_delete_flows(self.data.db_session()).await?,
                NodeDeleteStep::DeleteFlowSteps => self.undo_delete_flow_steps(self.data.db_session()).await?,
                NodeDeleteStep::DeleteIos => self.undo_delete_ios(self.data.db_session()).await?,
                NodeDeleteStep::DeleteDescriptions => self.undo_delete_descriptions(self.data.db_session()).await?,
                NodeDeleteStep::DeleteCounterData => self.undo_delete_counter_data(self.data.db_session()).await?,
                NodeDeleteStep::DeleteLikes => self.undo_delete_likes().await?,
                NodeDeleteStep::DeleteAttachments => self.undo_delete_attachments().await?,
                NodeDeleteStep::DeleteElasticData => self.undo_delete_elastic_data().await,
                NodeDeleteStep::Finish => (),
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
            .chunked_insert(db_session, &archive_nodes, 100)
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
            .chunked_delete(db_session, &archive_nodes, 100)
            .await
            .map_err(NodecosmosError::from)
    }

    async fn archive_workflows(&self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        let mut archive_workflows = vec![];

        for workflow in &self.deleted_workflows {
            archive_workflows.push(ArchivedWorkflow::from(workflow));
        }

        ArchivedWorkflow::unlogged_batch()
            .chunked_insert(db_session, &archive_workflows, 100)
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
            .chunked_delete(db_session, &archive_workflows, 100)
            .await
            .map_err(NodecosmosError::from)
    }

    async fn archive_flows(&self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        let mut archive_flows = vec![];

        for flow in &self.deleted_flows {
            archive_flows.push(ArchivedFlow::from(flow));
        }

        ArchivedFlow::unlogged_batch()
            .chunked_insert(db_session, &archive_flows, 100)
            .await
            .map_err(NodecosmosError::from)
    }

    async fn undo_archive_flows(&self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        let mut archive_flows = vec![];

        for flow in &self.deleted_flows {
            archive_flows.push(PkArchivedFlow::from(flow));
        }

        PkArchivedFlow::unlogged_batch()
            .chunked_delete(db_session, &archive_flows, 100)
            .await
            .map_err(NodecosmosError::from)
    }

    async fn archive_flow_steps(&self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        let mut archive_flow_steps = vec![];

        for flow_step in &self.deleted_flow_steps {
            archive_flow_steps.push(ArchivedFlowStep::from(flow_step));
        }

        ArchivedFlowStep::unlogged_batch()
            .chunked_insert(db_session, &archive_flow_steps, 100)
            .await
            .map_err(NodecosmosError::from)
    }

    async fn undo_archive_flow_steps(&self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        let mut archive_flow_steps = vec![];

        for flow_step in &self.deleted_flow_steps {
            archive_flow_steps.push(PkArchivedFlowStep::from(flow_step));
        }

        PkArchivedFlowStep::unlogged_batch()
            .chunked_delete(db_session, &archive_flow_steps, 100)
            .await
            .map_err(NodecosmosError::from)
    }

    async fn archive_ios(&self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        let mut archive_ios = vec![];

        for io in &self.deleted_ios {
            archive_ios.push(ArchivedIo::from(io));
        }

        ArchivedIo::unlogged_batch()
            .chunked_insert(db_session, &archive_ios, 100)
            .await
            .map_err(NodecosmosError::from)
    }

    async fn undo_archive_ios(&self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        let mut archive_ios = vec![];

        for io in &self.deleted_ios {
            archive_ios.push(PkArchivedIo::from(io));
        }

        PkArchivedIo::unlogged_batch()
            .chunked_delete(db_session, &archive_ios, 100)
            .await
            .map_err(NodecosmosError::from)
    }

    async fn archive_descriptions(&self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        let mut archive_descriptions = vec![];

        for description in &self.deleted_descriptions {
            archive_descriptions.push(ArchivedDescription::from(description));
        }

        ArchivedDescription::unlogged_batch()
            .chunked_insert(db_session, &archive_descriptions, 100)
            .await
            .map_err(NodecosmosError::from)
    }

    async fn undo_archive_descriptions(&self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        let mut archive_descriptions = vec![];

        for description in &self.deleted_descriptions {
            archive_descriptions.push(PkArchivedDescription::from(description));
        }

        PkArchivedDescription::unlogged_batch()
            .chunked_delete(db_session, &archive_descriptions, 100)
            .await
            .map_err(NodecosmosError::from)
    }

    async fn delete_nodes(&self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        Node::unlogged_batch()
            .chunked_delete(db_session, &self.deleted_nodes, 100)
            .await
            .map_err(NodecosmosError::from)
    }

    async fn undo_delete_nodes(&self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        Node::unlogged_batch()
            .chunked_insert(db_session, &self.deleted_nodes, 100)
            .await
            .map_err(NodecosmosError::from)
    }

    async fn delete_descendants(&self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        NodeDescendant::unlogged_batch()
            .chunked_delete(db_session, &self.deleted_descendants, 100)
            .await
            .map_err(NodecosmosError::from)
    }

    async fn undo_delete_descendants(&self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        NodeDescendant::unlogged_batch()
            .chunked_insert(db_session, &self.deleted_descendants, 100)
            .await
            .map_err(NodecosmosError::from)
    }

    async fn delete_workflows(&self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        Workflow::unlogged_batch()
            .chunked_delete(db_session, &self.deleted_workflows, 100)
            .await
            .map_err(NodecosmosError::from)
    }

    async fn undo_delete_workflows(&self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        Workflow::unlogged_batch()
            .chunked_insert(db_session, &self.deleted_workflows, 100)
            .await
            .map_err(NodecosmosError::from)
    }

    async fn delete_flows(&self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        Flow::unlogged_batch()
            .chunked_delete(db_session, &self.deleted_flows, 100)
            .await
            .map_err(NodecosmosError::from)
    }

    async fn undo_delete_flows(&self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        Flow::unlogged_batch()
            .chunked_insert(db_session, &self.deleted_flows, 100)
            .await
            .map_err(NodecosmosError::from)
    }

    async fn delete_flow_steps(&self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        FlowStep::unlogged_batch()
            .chunked_delete(db_session, &self.deleted_flow_steps, 100)
            .await
            .map_err(NodecosmosError::from)
    }

    async fn undo_delete_flow_steps(&self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        FlowStep::unlogged_batch()
            .chunked_insert(db_session, &self.deleted_flow_steps, 100)
            .await
            .map_err(NodecosmosError::from)
    }

    async fn delete_ios(&self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        Io::unlogged_batch()
            .chunked_delete(db_session, &self.deleted_ios, 100)
            .await
            .map_err(NodecosmosError::from)
    }

    async fn undo_delete_ios(&self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        Io::unlogged_batch()
            .chunked_insert(db_session, &self.deleted_ios, 100)
            .await
            .map_err(NodecosmosError::from)
    }

    async fn delete_descriptions(&self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        Description::unlogged_batch()
            .chunked_delete(db_session, &self.deleted_descriptions, 100)
            .await
            .map_err(NodecosmosError::from)
    }

    async fn undo_delete_descriptions(&self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        Description::unlogged_batch()
            .chunked_insert(db_session, &self.deleted_descriptions, 100)
            .await
            .map_err(NodecosmosError::from)
    }

    async fn delete_counter_data(&self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        NodeCounter::unlogged_batch()
            .chunked_delete(db_session, &self.deleted_counter_data, 100)
            .await
            .map_err(NodecosmosError::from)
    }

    async fn undo_delete_counter_data(&self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        NodeCounter::unlogged_batch()
            .chunked_insert(db_session, &self.deleted_counter_data, 100)
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
                branch_id: self.node.branchise_id(*id),
                ..Default::default()
            });
        }

        let _ = Like::unlogged_batch()
            .chunked_delete_by_partition_key(db_session, &deleted_likes, 100)
            .await
            .map_err(|e| log::error!("Error deleting likes: {:?}", e));

        Ok(())
    }

    // not critical for delete/merge
    async fn undo_delete_likes(&self) -> Result<(), NodecosmosError> {
        Ok(())
    }

    async fn delete_elastic_data(&self) {
        if self.node.is_original() {
            Node::bulk_delete_elastic_documents(self.data.elastic_client(), &self.deleted_node_ids).await;
        }
    }

    async fn undo_delete_elastic_data(&self) {
        if self.node.is_original() {
            Node::bulk_insert_elastic_documents(self.data.elastic_client(), &self.deleted_nodes).await;
        }
    }

    // not critical for delete/merge
    pub async fn delete_attachments(&mut self) -> Result<(), NodecosmosError> {
        let attachments = if self.node.is_branched() {
            Attachment::find_by_node_ids_and_branch_id(
                self.data.db_session(),
                &self.deleted_node_ids,
                self.node.branch_id,
            )
            .await?
        } else {
            Attachment::find_by_node_ids(self.data.db_session(), &self.deleted_node_ids).await?
        };

        attachments.delete_all(self.data)?;

        Ok(())
    }

    // not critical for delete/merge
    pub async fn undo_delete_attachments(&mut self) -> Result<(), NodecosmosError> {
        Ok(())
    }
}
