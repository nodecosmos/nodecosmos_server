use crate::models::helpers::set_updated_at_cb;
use crate::models::udts::{Creator, Owner};
use charybdis::*;
use chrono::Utc;
use scylla::batch::Batch;

#[partial_model_generator]
#[charybdis_model(
    table_name = "nodes",
    partition_keys = ["root_id"],
    clustering_keys = ["id"],
    secondary_indexes = ["id"]
)]
pub struct Node {
    // descendable
    #[serde(default)]
    pub id: Uuid,

    #[serde(default, rename = "rootId")]
    pub root_id: Uuid,

    #[serde(rename = "parentId")]
    pub parent_id: Option<Uuid>,

    #[serde(rename = "childIds")]
    pub child_ids: Option<List<Uuid>>,

    #[serde(rename = "descendantIds")]
    pub descendant_ids: Option<Set<Uuid>>,

    #[serde(rename = "ancestorIds")]
    pub ancestor_ids: Option<Set<Uuid>>,

    // node
    pub title: Option<Text>,
    pub description: Option<Text>,

    #[serde(rename = "descriptionMarkdown")]
    pub description_markdown: Option<Text>,

    // owners
    #[serde(rename = "ownerId")]
    pub owner_id: Option<Uuid>,

    #[serde(rename = "editorIds")]
    pub editor_ids: Option<Set<Uuid>>,

    pub owner: Option<Owner>,
    pub creator: Option<Creator>,

    // timestamps
    #[serde(rename = "createdAt")]
    pub created_at: Option<Timestamp>,

    #[serde(rename = "updatedAt")]
    pub updated_at: Option<Timestamp>,

    #[serde(rename = "likesCount")]
    pub likes_count: Option<BigInt>,
}

impl Node {
    pub async fn parent(&self, db_session: &CachingSession) -> Option<Node> {
        match self.parent_id {
            Some(parent_id) => {
                let mut parent = Node::new();
                parent.id = parent_id;
                parent.root_id = self.root_id;

                let parent = parent.find_by_primary_key(&db_session).await;

                match parent {
                    Ok(parent) => Some(parent),
                    Err(_) => None,
                }
            }
            None => None,
        }
    }

    pub fn set_defaults(&mut self) {
        self.id = Uuid::new_v4();
        self.created_at = Some(Utc::now());
        self.updated_at = Some(Utc::now());

        if self.root_id == Uuid::nil() {
            self.root_id = self.id;
        }
    }

    pub fn set_owner(&mut self, owner: Owner) {
        self.owner = Some(owner);
    }

    pub fn set_owner_id(&mut self, owner_id: Uuid) {
        self.owner_id = Some(owner_id);
    }

    pub fn set_editor_ids(&mut self, editor_ids: Option<Set<Uuid>>) {
        self.editor_ids = editor_ids;
    }

    pub fn set_ancestor_ids(&mut self, ancestor_ids: Set<Uuid>) {
        self.ancestor_ids = Some(ancestor_ids);
    }

    pub async fn append_to_parent_children(
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

    pub async fn delete_descendants(
        &self,
        db_session: &CachingSession,
    ) -> Result<(), CharybdisError> {
        match &self.descendant_ids {
            // TODO: run one by one in order to trigger cbs for each
            Some(descendant_ids) => {
                let mut batch: Batch = Default::default();
                let mut values = Vec::with_capacity(descendant_ids.len());

                for descendant_id in descendant_ids {
                    let query = Node::DELETE_QUERY;
                    batch.append_statement(query);
                    values.push((self.root_id, descendant_id));
                }

                db_session.batch(&batch, values).await?;

                Ok(())
            }
            None => Ok(()),
        }
    }
}

impl Callbacks for Node {
    async fn before_insert(&mut self, _session: &CachingSession) -> Result<(), CharybdisError> {
        self.set_defaults();

        Ok(())
    }

    async fn after_insert(&mut self, db_session: &CachingSession) -> Result<(), CharybdisError> {
        self.append_to_parent_children(db_session).await?;
        self.push_to_ancestors(db_session).await?;

        Ok(())
    }

    async fn before_update(&mut self, _session: &CachingSession) -> Result<(), CharybdisError> {
        self.updated_at = Some(Utc::now());

        Ok(())
    }

    async fn after_delete(&mut self, db_session: &CachingSession) -> Result<(), CharybdisError> {
        self.pull_from_parent_children(db_session).await?;
        self.pull_from_ancestors(db_session).await?;
        self.delete_descendants(db_session).await?;

        // TODO: remove likes, workflow & workflow_steps

        Ok(())
    }
}

partial_node!(GetNode, root_id, id, descendant_ids);

partial_node!(UpdateNodeTitle, root_id, id, title, updated_at);
set_updated_at_cb!(UpdateNodeTitle);

partial_node!(
    UpdateNodeDescription,
    root_id,
    id,
    description,
    description_markdown,
    updated_at
);
set_updated_at_cb!(UpdateNodeDescription);

partial_node!(UpdateNodeOwner, root_id, id, owner_id, updated_at);
set_updated_at_cb!(UpdateNodeOwner);

partial_node!(UpdateNodeLikesCount, root_id, id, likes_count, updated_at);
set_updated_at_cb!(UpdateNodeLikesCount);
