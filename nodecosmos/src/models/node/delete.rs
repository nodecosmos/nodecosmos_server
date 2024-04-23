use std::collections::HashMap;

use charybdis::batch::ModelBatch;
use charybdis::operations::Delete;
use charybdis::types::{Set, Uuid};
use futures::StreamExt;
use log::error;
use scylla::CachingSession;

use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::archived_description::ArchivedDescription;
use crate::models::archived_flow::ArchivedFlow;
use crate::models::archived_flow_step::ArchivedFlowStep;
use crate::models::archived_io::ArchivedIo;
use crate::models::archived_node::ArchivedNode;
use crate::models::archived_workflow::ArchivedWorkflow;
use crate::models::attachment::Attachment;
use crate::models::branch::update::BranchUpdate;
use crate::models::branch::Branch;
use crate::models::description::Description;
use crate::models::flow::{DeleteFlow, Flow};
use crate::models::flow_step::{DeleteFlowStep, FlowStep};
use crate::models::io::{DeleteIo, Io};
use crate::models::like::Like;
use crate::models::node::{Node, PrimaryKeyNode};
use crate::models::node_counter::NodeCounter;
use crate::models::node_descendant::NodeDescendant;
use crate::models::traits::Descendants;
use crate::models::traits::RefCloned;
use crate::models::traits::{Branchable, ElasticDocument, Pluck};
use crate::models::workflow::{DeleteWorkflow, Workflow};

impl Node {
    pub async fn delete_related_data(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_original() {
            NodeDelete::new(self, &data).run().await?;
        } else {
            if Branch::contains_created_node(data.db_session(), self.branch_id, self.id).await?
                || Self::is_original_deleted(data.db_session(), self.id).await?
            {
                NodeDelete::new(self, &data).run().await?;
            }
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

type Id = Uuid;
type OrderIndex = f64;
type Children = Vec<(Id, OrderIndex)>;
type ChildrenByParentId = HashMap<Uuid, Children>;

pub struct NodeDelete<'a> {
    data: &'a RequestData,
    node: &'a Node,
    node_ids_to_delete: Set<Uuid>,
    children_by_parent_id: ChildrenByParentId,
}

impl<'a> NodeDelete<'a> {
    pub fn new(node: &'a Node, data: &'a RequestData) -> NodeDelete<'a> {
        let mut node_ids_to_delete = Set::new();
        node_ids_to_delete.insert(node.id);

        Self {
            data,
            node,
            node_ids_to_delete,
            children_by_parent_id: ChildrenByParentId::new(),
        }
    }

    pub async fn run(&mut self) -> Result<(), NodecosmosError> {
        self.populate_delete_data().await.map_err(|err| {
            error!("populate_delete_data: {}", err);
            return err;
        })?;

        self.delete_descendants().await.map_err(|err| {
            error!("delete_descendants: {}", err);
            return err;
        })?;

        self.archive_related_data(self.data.db_session()).await.map_err(|err| {
            error!("archive_related_data: {}", err);
            return err;
        })?;

        self.delete_related_data().await.map_err(|err| {
            error!("delete_related_data: {}", err);
            return err;
        })?;

        self.delete_counter_data().await.map_err(|err| {
            error!("delete_counter_data: {}", err);
            return err;
        })?;

        self.delete_attachments().await.map_err(|err| {
            error!("delete_attachments: {}", err);
            return err;
        })?;

        self.delete_elastic_data().await;

        Ok(())
    }

    async fn populate_delete_data(&mut self) -> Result<(), NodecosmosError> {
        let mut descendants = self.node.descendants(self.data.db_session()).await?;

        while let Some(descendant) = descendants.next().await {
            let descendant = descendant?;

            self.node_ids_to_delete.insert(descendant.id);

            if !self.node.is_root {
                self.children_by_parent_id
                    .entry(descendant.parent_id)
                    .or_default()
                    .push((descendant.id, descendant.order_index));
            }
        }

        Ok(())
    }

    async fn archived_nodes(&mut self, db_session: &CachingSession) -> Result<Vec<ArchivedNode>, NodecosmosError> {
        let mut nodes = if self.node.is_branched() {
            Node::find_by_ids_and_branch_id(db_session, &self.node_ids_to_delete, self.node.branch_id).await?
        } else {
            Node::find_by_id(self.node.id).execute(db_session).await?
        };

        let mut archive_nodes: Vec<ArchivedNode> = vec![];

        while let Some(node) = nodes.next().await {
            let node = node?;

            if node.is_branched() && self.node.is_original() {
                continue;
            }

            archive_nodes.push(ArchivedNode::from(node));
        }

        Ok(archive_nodes)
    }

    async fn archived_workflows(
        &mut self,
        db_session: &CachingSession,
    ) -> Result<Vec<ArchivedWorkflow>, NodecosmosError> {
        let mut workflows = if self.node.is_branched() {
            Workflow::find_by_node_ids_and_branch_id(db_session, &self.node_ids_to_delete, self.node.branch_id).await?
        } else {
            Workflow::find_by_node_id(self.node.id).execute(db_session).await?
        };

        let mut archive_workflows: Vec<ArchivedWorkflow> = vec![];

        while let Some(workflow) = workflows.next().await {
            let workflow = workflow?;

            if workflow.is_branched() && self.node.is_original() {
                continue;
            }

            let archive = ArchivedWorkflow::from(workflow);
            archive_workflows.push(archive);
        }

        Ok(archive_workflows)
    }

    async fn archived_flows(&mut self, db_session: &CachingSession) -> Result<Vec<ArchivedFlow>, NodecosmosError> {
        let mut flows = if self.node.is_branched() {
            Flow::find_by_node_ids_and_branch_id(db_session, &self.node_ids_to_delete, self.node.branch_id).await?
        } else {
            Flow::find_by_node_id_and_branch_id(self.node.id, self.node.id)
                .execute(db_session)
                .await?
        };

        let mut archive_flows: Vec<ArchivedFlow> = vec![];

        while let Some(flow) = flows.next().await {
            let flow = flow?;

            let archive = ArchivedFlow::from(flow);

            archive_flows.push(archive);
        }

        Ok(archive_flows)
    }

    async fn archived_flow_steps(
        &mut self,
        db_session: &CachingSession,
    ) -> Result<Vec<ArchivedFlowStep>, NodecosmosError> {
        let mut flow_steps = if self.node.is_branched() {
            FlowStep::find_by_node_ids_and_branch_id(db_session, &self.node_ids_to_delete, self.node.branch_id).await?
        } else {
            FlowStep::find_by_node_id_and_branch_id(self.node.id, self.node.id)
                .execute(db_session)
                .await?
        };

        let mut archive_flow_steps: Vec<ArchivedFlowStep> = vec![];

        while let Some(flow_step) = flow_steps.next().await {
            let flow_step = flow_step?;

            let archive = ArchivedFlowStep::from(flow_step);

            archive_flow_steps.push(archive);
        }

        Ok(archive_flow_steps)
    }

    async fn archived_ios(&mut self, db_session: &CachingSession) -> Result<Vec<ArchivedIo>, NodecosmosError> {
        let mut ios = if self.node.is_branched() {
            Io::find_by_root_id_and_branch_id(self.node.root_id, self.node.branch_id)
                .execute(db_session)
                .await?
        } else {
            Io::find_by_root_id_and_branch_id(self.node.root_id, self.node.root_id)
                .execute(db_session)
                .await?
        };

        let mut archive_ios: Vec<ArchivedIo> = vec![];

        while let Some(io) = ios.next().await {
            let io = io?;

            if !self.node_ids_to_delete.contains(&io.node_id) {
                continue;
            }

            let archive = ArchivedIo::from(io);

            archive_ios.push(archive);
        }

        Ok(archive_ios)
    }

    async fn archived_descriptions(
        &mut self,
        db_session: &CachingSession,
    ) -> Result<Vec<ArchivedDescription>, NodecosmosError> {
        let mut descriptions = Description::find_by_node_ids(db_session, &self.node_ids_to_delete).await?;
        let mut archive_descriptions: Vec<ArchivedDescription> = vec![];

        while let Some(description) = descriptions.next().await {
            let description = description?;

            if description.is_branched() && self.node.is_original() {
                continue;
            }

            let archive = ArchivedDescription::from(description);

            archive_descriptions.push(archive);
        }

        Ok(archive_descriptions)
    }

    // here we find all related data for node and its descendants, and archive it
    // meaning that e.g. for Node we create ArchivedNode record, for Flow we create ArchivedFlow record etc.
    async fn archive_related_data(&mut self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        let archive_nodes = self.archived_nodes(db_session).await?;
        let archived_workflows = self.archived_workflows(db_session).await?;
        let archived_flows = self.archived_flows(db_session).await?;
        let archived_flow_steps = self.archived_flow_steps(db_session).await?;
        let archived_ios = self.archived_ios(db_session).await?;
        let archived_descriptions = self.archived_descriptions(db_session).await?;

        ArchivedNode::unlogged_batch()
            .chunked_insert(db_session, &archive_nodes, 100)
            .await?;
        ArchivedWorkflow::unlogged_batch()
            .chunked_insert(db_session, &archived_workflows, 100)
            .await?;
        ArchivedFlow::unlogged_batch()
            .chunked_insert(db_session, &archived_flows, 100)
            .await?;
        ArchivedFlowStep::unlogged_batch()
            .chunked_insert(db_session, &archived_flow_steps, 100)
            .await?;
        ArchivedIo::unlogged_batch()
            .chunked_insert(db_session, &archived_ios, 100)
            .await?;
        ArchivedDescription::unlogged_batch()
            .chunked_insert(db_session, &archived_descriptions, 100)
            .await?;

        Ok(())
    }

    /// Here we delete workflows, flows, flow_steps, ios, likes, like_counts for node and its descendants.
    pub async fn delete_related_data(&mut self) -> Result<(), NodecosmosError> {
        let mut delete_workflows: Vec<DeleteWorkflow> = vec![];
        let mut delete_flows: Vec<DeleteFlow> = vec![];
        let mut delete_flow_steps: Vec<DeleteFlowStep> = vec![];
        let mut delete_ios: Vec<DeleteIo> = vec![];
        let mut delete_likes: Vec<Like> = vec![];
        let mut delete_descriptions: Vec<Description> = vec![];
        let mut delete_pk_nodes: Vec<PrimaryKeyNode> = vec![];

        for id in self.node_ids_to_delete.iter() {
            delete_workflows.push(DeleteWorkflow {
                node_id: *id,
                branch_id: *id,
                ..Default::default()
            });
            delete_flows.push(DeleteFlow {
                node_id: *id,
                branch_id: *id,
                ..Default::default()
            });
            delete_flow_steps.push(DeleteFlowStep {
                node_id: *id,
                branch_id: *id,
                ..Default::default()
            });
            delete_ios.push(DeleteIo {
                root_id: self.node.root_id,
                branch_id: *id,
                ..Default::default()
            });

            delete_likes.push(Like {
                object_id: *id,
                branch_id: *id,
                ..Default::default()
            });

            delete_descriptions.push(Description {
                object_id: *id,
                branch_id: *id,
                ..Default::default()
            });

            delete_pk_nodes.push(PrimaryKeyNode {
                id: *id,
                branch_id: *id,
                ..Default::default()
            });
        }

        DeleteWorkflow::unlogged_batch()
            .chunked_delete(self.data.db_session(), &delete_workflows, 100)
            .await?;
        DeleteFlow::unlogged_batch()
            .chunked_delete_by_partition_key(self.data.db_session(), &delete_flows, 100)
            .await?;
        DeleteFlowStep::unlogged_batch()
            .chunked_delete_by_partition_key(self.data.db_session(), &delete_flow_steps, 100)
            .await?;
        DeleteIo::unlogged_batch()
            .chunked_delete_by_partition_key(self.data.db_session(), &delete_ios, 100)
            .await?;
        Like::unlogged_batch()
            .chunked_delete_by_partition_key(self.data.db_session(), &delete_likes, 100)
            .await?;
        Description::unlogged_batch()
            .chunked_delete(self.data.db_session(), &delete_descriptions, 100)
            .await?;
        PrimaryKeyNode::unlogged_batch()
            .chunked_delete(self.data.db_session(), &delete_pk_nodes, 100)
            .await?;

        Ok(())
    }

    //  Counter and non-counter mutations cannot exist in the same batch
    pub async fn delete_counter_data(&mut self) -> Result<(), NodecosmosError> {
        for node_ids_chunk in self
            .node_ids_to_delete
            .clone()
            .into_iter()
            .collect::<Vec<Uuid>>()
            .chunks(100)
        {
            let mut batch = NodeCounter::unlogged_batch();

            for id in node_ids_chunk {
                batch.append_delete(&NodeCounter {
                    id: *id,
                    branch_id: self.node.branchise_id(*id),
                    ..Default::default()
                });
            }

            batch.execute(self.data.db_session()).await.map_err(|err| {
                return NodecosmosError::InternalServerError(format!(
                    "NodeDelete::delete_related_data: batch.execute: {}",
                    err
                ));
            })?;
        }

        Ok(())
    }

    pub async fn delete_attachments(&mut self) -> Result<(), NodecosmosError> {
        Attachment::delete_by_node_ids(self.data, &self.node_ids_to_delete).await?;

        Ok(())
    }

    /// Here we delete all of node descendants for all of its ancestors.
    /// We use child_ids_by_parent_id so we don't have to query ancestor_ids for each descendant node.
    /// Each node in the tree has its own descendant records in node_descendants table,
    /// so we have to get all of them.
    pub async fn delete_descendants(&mut self) -> Result<(), NodecosmosError> {
        if self.node.is_root {
            self.delete_tree().await?;
            return Ok(());
        }

        match self.node.parent_id {
            Some(parent_id) => {
                let order_index = self.node.order_index;

                self.children_by_parent_id
                    .entry(parent_id)
                    .or_default()
                    .push((self.node.id, order_index));

                let mut delete_stack = vec![parent_id];
                let mut current_ancestor_ids = self.node.ancestor_ids.ref_cloned();

                while let Some(parent_id) = delete_stack.pop() {
                    current_ancestor_ids.insert(parent_id);

                    let children: Children = self.children_by_parent_id.get(&parent_id).unwrap_or(&vec![]).clone();

                    for children_chunk in children.chunks(100) {
                        let mut batch = NodeDescendant::unlogged_delete_batch();

                        for (child_id, order_index) in children_chunk {
                            // delete node descendants for all of its ancestors
                            for ancestor_id in &current_ancestor_ids {
                                let branch_id = self.node.branchise_id(*ancestor_id);

                                batch.append_delete(&NodeDescendant {
                                    root_id: self.node.root_id,
                                    branch_id,
                                    node_id: *ancestor_id,
                                    order_index: *order_index,
                                    id: *child_id,
                                    ..Default::default()
                                });
                            }

                            // populate delete stack with child ids
                            delete_stack.push(*child_id);
                        }

                        batch.execute(self.data.db_session()).await.map_err(|err| {
                            return NodecosmosError::InternalServerError(format!(
                                "NodeDelete::delete_descendants: batch.execute: {}",
                                err
                            ));
                        })?;
                    }
                }
            }
            None => {
                return Err(NodecosmosError::InternalServerError(
                    "NodeDelete::delete_descendants: parent_id is None".to_string(),
                ));
            }
        };

        Ok(())
    }

    async fn delete_tree(&self) -> Result<(), NodecosmosError> {
        NodeDescendant {
            root_id: self.node.id,
            ..Default::default()
        }
        .delete_by_partition_key()
        .execute(self.data.db_session())
        .await?;

        return Ok(());
    }

    async fn delete_elastic_data(&self) {
        if self.node.is_original() {
            Node::bulk_delete_elastic_documents(
                self.data.elastic_client(),
                &self.node_ids_to_delete.clone().into_iter().collect(),
            )
            .await;
        }
    }
}
