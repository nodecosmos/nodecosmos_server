use crate::app::CbExtension;
use crate::elastic::{
    add_elastic_document, bulk_delete_elastic_documents, delete_elastic_document,
    update_elastic_document,
};
use crate::models::flow::FlowDelete;
use crate::models::flow_step::FlowStepDelete;
use crate::models::helpers::{
    default_to_0, default_to_false, impl_node_updated_at_with_elastic_ext_cb, impl_updated_at_cb,
    sanitize_description_ext_cb_fn,
};
use crate::models::input_output::IoDelete;
use crate::models::udts::{Creator, Owner};
use crate::models::workflow::{Workflow, WorkflowDelete};
use charybdis::*;
use chrono::Utc;
use scylla::batch::Batch;

#[partial_model_generator]
#[charybdis_model(
    table_name = nodes,
    partition_keys = [root_id],
    clustering_keys = [id],
    secondary_indexes = [id],
)]
pub struct Node {
    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,

    #[serde(default, rename = "rootId")]
    pub root_id: Uuid,

    #[serde(rename = "isPublic")]
    pub is_public: Option<Boolean>,

    #[serde(rename = "isRoot", default = "default_to_false")]
    pub is_root: Option<Boolean>,

    #[serde(rename = "parentId")]
    pub parent_id: Option<Uuid>,

    #[serde(rename = "childIds")]
    pub child_ids: Option<List<Uuid>>,

    #[serde(rename = "descendantIds")]
    pub descendant_ids: Option<List<Uuid>>,

    #[serde(rename = "ancestorIds")]
    pub ancestor_ids: Option<List<Uuid>>,

    // node
    pub title: Option<Text>,
    pub description: Option<Text>,

    #[serde(rename = "shortDescription")]
    pub short_description: Option<Text>,

    #[serde(rename = "descriptionMarkdown")]
    pub description_markdown: Option<Text>,

    #[serde(rename = "ownerId")]
    pub owner_id: Option<Uuid>,

    #[serde(rename = "ownerType")]
    pub owner_type: Option<Text>,

    #[serde(rename = "creatorId")]
    pub creator_id: Option<Uuid>,

    #[serde(rename = "editorIds")]
    pub editor_ids: Option<Set<Uuid>>,

    pub owner: Option<Owner>, // for front-end compatibility
    pub creator: Option<Creator>,

    // timestamps
    #[serde(rename = "createdAt")]
    pub created_at: Option<Timestamp>,

    #[serde(rename = "updatedAt")]
    pub updated_at: Option<Timestamp>,

    #[serde(rename = "likesCount", default = "default_to_0")]
    pub likes_count: Option<BigInt>,
}

impl Node {
    pub const ELASTIC_IDX_NAME: &'static str = "nodes";

    pub fn set_owner(&mut self, owner: Owner) {
        self.owner_id = Some(owner.id);
        self.owner_type = Some(owner.owner_type.clone());

        self.owner = Some(owner);
    }

    pub async fn parent(&self, db_session: &CachingSession) -> Option<Node> {
        match self.parent_id {
            Some(parent_id) => {
                let mut parent = Node::new();
                parent.id = parent_id;
                parent.root_id = self.root_id;

                let parent = parent.find_by_primary_key(db_session).await;

                match parent {
                    Ok(parent) => Some(parent),
                    Err(_) => None,
                }
            }
            None => None,
        }
    }

    pub async fn set_defaults(&mut self) -> Result<(), CharybdisError> {
        if self.root_id == Uuid::nil() {
            self.root_id = self.id;
        }

        self.created_at = Some(Utc::now());
        self.updated_at = Some(Utc::now());
        self.is_root = Some(self.parent_id.is_none());

        Ok(())
    }

    pub async fn push_to_parent_children(
        &mut self,
        db_session: &CachingSession,
    ) -> Result<(), CharybdisError> {
        if let Some(parent_id) = &self.parent_id {
            let query = Node::PUSH_TO_CHILD_IDS_QUERY;
            execute(db_session, query, (self.id, self.root_id, parent_id)).await?;
        }

        Ok(())
    }

    pub async fn push_to_ancestors(
        &mut self,
        db_session: &CachingSession,
    ) -> Result<(), CharybdisError> {
        if let Some(ancestor_ids) = &self.ancestor_ids {
            let mut batch: Batch = Default::default();
            let mut values = Vec::with_capacity(ancestor_ids.len());

            for ancestor_id in ancestor_ids {
                let query = Node::PUSH_TO_DESCENDANT_IDS_QUERY;
                batch.append_statement(query);
                values.push((self.id, self.root_id, ancestor_id));
            }

            db_session.batch(&batch, values).await?;
        }

        Ok(())
    }

    pub async fn pull_from_parent_children(
        &mut self,
        db_session: &CachingSession,
    ) -> Result<(), CharybdisError> {
        if let Some(parent_id) = &self.parent_id {
            let query = Node::PULL_FROM_CHILD_IDS_QUERY;
            execute(db_session, query, (self.id, self.root_id, parent_id)).await?;
        }

        Ok(())
    }

    pub async fn pull_from_ancestors(
        &mut self,
        db_session: &CachingSession,
    ) -> Result<(), CharybdisError> {
        if let Some(ancestor_ids) = &self.ancestor_ids {
            let mut batch: Batch = Default::default();
            let mut values = Vec::with_capacity(ancestor_ids.len());

            for ancestor_id in ancestor_ids {
                let query = Node::PULL_FROM_DESCENDANT_IDS_QUERY;
                batch.append_statement(query);
                values.push((self.id, self.root_id, ancestor_id));
            }

            db_session.batch(&batch, values).await?;
        }

        Ok(())
    }

    pub async fn delete_workflows(
        &self,
        db_session: &CachingSession,
    ) -> Result<(), CharybdisError> {
        let workflows = Workflow {
            node_id: self.id,
            ..Default::default()
        }
        .find_by_partition_key(db_session)
        .await?;

        for workflow in workflows {
            workflow?.delete_cb(db_session).await?;
        }

        Ok(())
    }

    // delete descendant nodes, workflows, flows, and flow steps as single batch
    pub async fn delete_descendants(
        &self,
        db_session: &CachingSession,
    ) -> Result<(), CharybdisError> {
        match &self.descendant_ids {
            Some(descendant_ids) => {
                let mut batch = CharybdisModelBatch::new();

                for descendant_id in descendant_ids {
                    let workflows_to_delete = WorkflowDelete {
                        node_id: *descendant_id,
                        ..Default::default()
                    }
                    .find_by_partition_key(db_session)
                    .await?;

                    let flows_to_delete = FlowDelete {
                        node_id: *descendant_id,
                        ..Default::default()
                    }
                    .find_by_partition_key(db_session)
                    .await?;

                    let flow_steps_to_delete = FlowStepDelete {
                        node_id: *descendant_id,
                        ..Default::default()
                    }
                    .find_by_partition_key(db_session)
                    .await?;

                    let ios_to_delete = IoDelete {
                        node_id: *descendant_id,
                        ..Default::default()
                    }
                    .find_by_partition_key(db_session)
                    .await?;

                    batch.append_deletes(ios_to_delete)?;
                    batch.append_deletes(flow_steps_to_delete)?;
                    batch.append_deletes(flows_to_delete)?;
                    batch.append_deletes(workflows_to_delete)?;
                    batch.append_delete(DeleteNode {
                        id: *descendant_id,
                        root_id: self.root_id,
                    })?;
                }

                batch.execute(db_session).await
            }
            None => Ok(()),
        }
    }

    pub async fn add_to_elastic_index(&self, ext: &CbExtension) {
        add_elastic_document(
            &ext.elastic_client,
            Node::ELASTIC_IDX_NAME,
            self,
            self.id.to_string(),
        )
        .await;
    }

    pub async fn delete_related_elastic_data(&self, ext: &CbExtension) {
        delete_elastic_document(
            &ext.elastic_client,
            Node::ELASTIC_IDX_NAME,
            self.id.to_string(),
        )
        .await;

        bulk_delete_elastic_documents(
            &ext.elastic_client,
            Node::ELASTIC_IDX_NAME,
            self.descendant_ids.clone().unwrap_or_default(),
        )
        .await;
    }
}

impl ExtCallbacks<CbExtension> for Node {
    async fn before_insert(
        &mut self,
        _db_session: &CachingSession,
        _ext: &CbExtension,
    ) -> Result<(), CharybdisError> {
        self.set_defaults().await?;

        Ok(())
    }

    async fn after_insert(
        &mut self,
        db_session: &CachingSession,
        ext: &CbExtension,
    ) -> Result<(), CharybdisError> {
        self.push_to_parent_children(db_session).await?;
        self.push_to_ancestors(db_session).await?;

        self.add_to_elastic_index(ext).await;

        Ok(())
    }

    async fn after_delete(
        &mut self,
        db_session: &CachingSession,
        ext: &CbExtension,
    ) -> Result<(), CharybdisError> {
        self.pull_from_parent_children(db_session).await?;
        self.pull_from_ancestors(db_session).await?;

        self.delete_descendants(db_session).await?;
        self.delete_workflows(db_session).await?;

        self.delete_related_elastic_data(ext).await;

        Ok(())
    }
}

partial_node!(
    BaseNode,
    root_id,
    id,
    short_description,
    ancestor_ids,
    child_ids,
    descendant_ids,
    title,
    parent_id,
    owner_id,
    editor_ids,
    likes_count,
    owner,
    created_at,
    updated_at
);

partial_node!(
    GetNodeDescription,
    root_id,
    id,
    description,
    description_markdown
);

partial_node!(UpdateNodeTitle, root_id, id, title, updated_at);
impl_node_updated_at_with_elastic_ext_cb!(UpdateNodeTitle);

partial_node!(
    UpdateNodeDescription,
    root_id,
    id,
    description,
    short_description,
    description_markdown,
    updated_at
);
impl ExtCallbacks<CbExtension> for UpdateNodeDescription {
    sanitize_description_ext_cb_fn!();

    async fn after_update(
        &mut self,
        _db_session: &CachingSession,
        ext: &CbExtension,
    ) -> Result<(), CharybdisError> {
        use ammonia::clean;

        if let Some(description) = &self.description {
            self.description = Some(clean(description));
        }

        if let Some(short_description) = &self.short_description {
            self.short_description = Some(clean(short_description));
        }

        update_elastic_document(
            &ext.elastic_client,
            Node::ELASTIC_IDX_NAME,
            self,
            self.id.to_string(),
        )
        .await;

        Ok(())
    }
}

partial_node!(UpdateNodeOwner, root_id, id, owner_id, updated_at);
impl_updated_at_cb!(UpdateNodeOwner);

partial_node!(UpdateNodeLikesCount, root_id, id, likes_count, updated_at);
impl_node_updated_at_with_elastic_ext_cb!(UpdateNodeLikesCount);

partial_node!(DeleteNode, root_id, id);
