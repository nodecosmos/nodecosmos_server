use charybdis::prelude::*;
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
    pub title: Option<Text>,
    pub description: Option<Text>,
    pub description_markdown: Option<Text>,
    // owners
    pub owner_id: Option<Uuid>,
    pub editor_ids: Option<Set<Uuid>>,
    // timestamps
    pub created_at: Option<Timestamp>,
    pub updated_at: Option<Timestamp>,
}

impl Node {
    pub fn set_defaults(&mut self) {
        self.id = Uuid::new_v4();
        self.created_at = Some(Timestamp::now());
        self.updated_at = Some(Timestamp::now());
    }

    pub async fn append_to_parent_children(
        &mut self,
        db_session: &CachingSession,
    ) -> Result<(), CharybdisError> {
        match &self.parent_id {
            Some(parent_id) => {
                let query = Node::PUSH_TO_CHILD_IDS_QUERY;
                db_session
                    .query(query, (self.id, self.root_id, parent_id))
                    .await?;

                Ok(())
            }
            None => Ok(()),
        }
    }

    pub async fn append_to_ancestors(
        &mut self,
        db_session: &CachingSession,
    ) -> Result<(), CharybdisError> {
        match &self.ancestor_ids {
            Some(ancestor_ids) => {
                let mut batch: Batch = Default::default();
                let mut values: Vec<Uuid> = Vec::with_capacity(ancestor_ids.len());

                for ancestor_id in ancestor_ids {
                    let query = Node::PUSH_TO_DESCENDANT_IDS_QUERY;
                    batch.append_statement(query);
                    values.push(*ancestor_id);
                }

                batch.execute(&db_session).await?;
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
        self.append_to_ancestors(db_session).await?;

        Ok(())
    }

    async fn before_update(&mut self, _session: &CachingSession) -> Result<(), CharybdisError> {
        self.updated_at = Some(Timestamp::now());

        Ok(())
    }

    async fn before_delete(&mut self, _session: &CachingSession) -> Result<(), CharybdisError> {
        Ok(())
    }
}

partial_node!(GetNode, id, root_id, descendant_ids);
