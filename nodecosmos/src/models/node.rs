use crate::models::udts::{Creator, Owner};
use charybdis::prelude::*;
use chrono::Utc;
use scylla::batch::Batch;

#[partial_model_generator]
#[charybdis_model(table_name = "nodes",
                  partition_keys = ["root_id"],
                  clustering_keys = ["id"],
                  secondary_indexes = [])]
pub struct Node {
    // descendable
    pub id: Uuid,
    pub root_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub child_ids: Option<Set<Uuid>>,
    pub descendant_ids: Option<Set<Uuid>>,
    pub ancestor_ids: Option<Set<Uuid>>,
    // node
    pub title: Option<Text>,
    pub description: Option<Text>,
    pub description_markdown: Option<Text>,
    // owners
    pub owner_id: Option<Uuid>,
    pub editor_ids: Option<Set<Uuid>>,
    pub owner: Option<Owner>,
    pub creator: Option<Creator>,
    // timestamps
    pub created_at: Option<Timestamp>,
    pub updated_at: Option<Timestamp>,
}

partial_node!(GetNode, id, root_id, descendant_ids);

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

        Ok(())
    }
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
    }

    pub fn set_editor_ids(&mut self, editor_ids: Set<Uuid>) {
        self.editor_ids = Some(editor_ids);
    }

    pub async fn append_to_parent_children(
        &mut self,
        db_session: &CachingSession,
    ) -> Result<(), CharybdisError> {
        match &self.parent_id {
            Some(parent_id) => {
                let query = Node::PUSH_TO_CHILD_IDS_QUERY;
                self.execute(db_session, query, (self.id, self.root_id, parent_id))
                    .await?;

                Ok(())
            }
            None => Ok(()),
        }
    }

    pub async fn push_to_ancestors(
        &mut self,
        db_session: &CachingSession,
    ) -> Result<(), CharybdisError> {
        match &self.ancestor_ids {
            Some(ancestor_ids) => {
                let mut batch: Batch = Default::default();
                let mut values = Vec::with_capacity(ancestor_ids.len());

                for ancestor_id in ancestor_ids {
                    let query = Node::PUSH_TO_DESCENDANT_IDS_QUERY;
                    batch.append_statement(query);
                    values.push((self.id, self.root_id, ancestor_id));
                }

                db_session.batch(&batch, values).await?;
                Ok(())
            }
            None => Ok(()),
        }
    }

    pub async fn pull_from_parent_children(
        &mut self,
        db_session: &CachingSession,
    ) -> Result<(), CharybdisError> {
        match &self.parent_id {
            Some(parent_id) => {
                let query = Node::PULL_FROM_CHILD_IDS_QUERY;
                self.execute(db_session, query, (self.id, self.root_id, parent_id))
                    .await?;

                Ok(())
            }
            None => Ok(()),
        }
    }

    pub async fn pull_from_ancestors(
        &mut self,
        db_session: &CachingSession,
    ) -> Result<(), CharybdisError> {
        match &self.ancestor_ids {
            Some(ancestor_ids) => {
                let mut batch: Batch = Default::default();
                let mut values = Vec::with_capacity(ancestor_ids.len());

                for ancestor_id in ancestor_ids {
                    let query = Node::PULL_FROM_DESCENDANT_IDS_QUERY;
                    batch.append_statement(query);
                    values.push((self.id, self.root_id, ancestor_id));
                }

                db_session.batch(&batch, values).await?;

                Ok(())
            }
            None => Ok(()),
        }
    }
}

partial_node!(UpdateNodeTitle, id, root_id, title, updated_at);

impl Callbacks for UpdateNodeTitle {
    async fn before_update(&mut self, _session: &CachingSession) -> Result<(), CharybdisError> {
        self.updated_at = Some(Utc::now());

        Ok(())
    }
}

partial_node!(
    UpdateNodeDescription,
    id,
    root_id,
    description,
    description_markdown,
    updated_at
);

impl Callbacks for UpdateNodeDescription {
    async fn before_update(&mut self, _session: &CachingSession) -> Result<(), CharybdisError> {
        self.updated_at = Some(Utc::now());

        Ok(())
    }
}

partial_node!(UpdateNodeOwner, id, root_id, owner_id, updated_at);

impl Callbacks for UpdateNodeOwner {
    async fn before_update(&mut self, _session: &CachingSession) -> Result<(), CharybdisError> {
        self.updated_at = Some(Utc::now());

        Ok(())
    }
}
