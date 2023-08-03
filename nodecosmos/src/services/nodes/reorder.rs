use crate::errors::NodecosmosError;
use crate::models::helpers::clone_ref::ClonedRef;
use crate::models::node::{find_node_query, Node, UpdateAncestors, UpdateChildIds, UpdateParent};
use actix_web::web::Data;
use charybdis::{execute, CharybdisModelBatch, Deserialize, Find, Update, Uuid};
use futures::StreamExt;
use scylla::CachingSession;

#[derive(Deserialize)]
pub struct ReorderParams {
    #[serde(rename = "rootId")]
    pub root_id: Uuid,

    #[serde(rename = "id")]
    pub id: Uuid,

    #[serde(rename = "newParentId")]
    pub new_parent_id: Uuid,

    #[serde(rename = "newSiblingIndex")]
    pub new_sibling_index: usize,
}

pub struct Reorderer {
    pub params: ReorderParams,
    pub root: Node,
    pub node: Node,
    pub new_parent: Node,
    pub new_node_ancestor_ids: Option<Vec<Uuid>>,
    pub db_session: Data<CachingSession>,
}

impl Reorderer {
    pub async fn new(
        db_session: Data<CachingSession>,
        params: ReorderParams,
    ) -> Result<Self, NodecosmosError> {
        let root = Node {
            root_id: params.root_id,
            id: params.root_id,
            ..Default::default()
        }
        .find_by_primary_key(&db_session)
        .await?;

        let node = Node {
            root_id: params.root_id,
            id: params.id,
            ..Default::default()
        }
        .find_by_primary_key(&db_session)
        .await?;

        let new_parent = Node {
            root_id: params.root_id,
            id: params.new_parent_id,
            ..Default::default()
        }
        .find_by_primary_key(&db_session)
        .await?;

        Ok(Self {
            params,
            root,
            node,
            new_parent,
            new_node_ancestor_ids: None,
            db_session,
        })
    }

    pub async fn reorder(&mut self) -> Result<(), NodecosmosError> {
        self.add_node_to_parent().await?;

        // I initially used batch statements for all the queries, but I ran into a
        // problem where the batch statements were not executed in the order they were
        // added to the batch. This caused the reorder to fail.
        if self.is_parent_changed() {
            self.add_remove_from_old_parent_statements().await?;
            self.add_remove_node_from_old_ancestors_statements().await?;
            self.add_remove_node_descendants_from_all_descendants_statements()
                .await?;

            self.build_new_ancestor_ids();
            self.add_new_parent_statement().await?;
            self.add_node_to_ancestor_descendants_and_op().await?;
            self.add_new_ancestors_to_descendants_statements_and_op()
                .await?;
        }

        Ok(())
    }

    fn is_parent_changed(&self) -> bool {
        if let Some(current_parent_id) = self.node.parent_id {
            return current_parent_id != self.params.new_parent_id;
        }

        false
    }

    fn build_new_ancestor_ids(&mut self) {
        let new_parent_ancestors = self.new_parent.ancestor_ids.cloned_ref();
        let mut new_ancestors = Vec::with_capacity(new_parent_ancestors.len() + 1);

        new_ancestors.extend(new_parent_ancestors);
        new_ancestors.push(self.new_parent.id);

        self.new_node_ancestor_ids = Some(new_ancestors);
    }

    async fn add_remove_from_old_parent_statements(&mut self) -> Result<(), NodecosmosError> {
        if let Some(parent_id) = &self.node.parent_id {
            let query = Node::PULL_FROM_CHILD_IDS_QUERY;
            execute(
                &self.db_session,
                query,
                (self.node.id, self.node.root_id, parent_id),
            )
            .await?;
        }

        Ok(())
    }

    async fn add_remove_node_from_old_ancestors_statements(
        &mut self,
    ) -> Result<(), NodecosmosError> {
        let ancestor_ids = self.node.ancestor_ids.cloned_ref();
        let mut batch = CharybdisModelBatch::new();

        for ancestor_id in ancestor_ids {
            let query = Node::PULL_FROM_DESCENDANT_IDS_QUERY;
            batch.append_statement(query, (self.node.id, self.node.root_id, ancestor_id))?;
        }

        batch.execute(&self.db_session).await?;
        Ok(())
    }

    async fn add_remove_node_descendants_from_all_descendants_statements(
        &mut self,
    ) -> Result<(), NodecosmosError> {
        let mut batch = CharybdisModelBatch::new();

        let root_descendant_ids = self.root.descendant_ids.cloned_ref();
        let node_descendant_ids = self.node.descendant_ids.cloned_ref();

        for descendant_id in node_descendant_ids {
            for ancestor_id in &root_descendant_ids {
                let query = Node::PULL_FROM_DESCENDANT_IDS_QUERY;
                batch.append_statement(query, (descendant_id, self.node.root_id, *ancestor_id))?;
            }
        }

        batch.execute(&self.db_session).await?;

        Ok(())
    }

    async fn add_new_parent_statement(&mut self) -> Result<(), NodecosmosError> {
        let update_parent_node = UpdateParent {
            root_id: self.node.root_id,
            id: self.node.id,
            parent_id: Some(self.params.new_parent_id),
        };

        update_parent_node.update(&self.db_session).await?;

        Ok(())
    }

    async fn add_node_to_parent(&mut self) -> Result<(), NodecosmosError> {
        let mut new_parent_child_ids = self.new_parent.child_ids.cloned_ref();

        if !self.is_parent_changed() {
            new_parent_child_ids.retain(|id| id != &self.node.id);
        }

        new_parent_child_ids.insert(self.params.new_sibling_index, self.node.id);

        let update_child_ids_node = UpdateChildIds {
            root_id: self.node.root_id,
            id: self.params.new_parent_id,
            child_ids: Some(new_parent_child_ids),
        };

        update_child_ids_node.update(&self.db_session).await?;

        Ok(())
    }

    async fn add_node_to_ancestor_descendants_and_op(&mut self) -> Result<(), NodecosmosError> {
        let mut batch = CharybdisModelBatch::new();

        let new_node_ancestor_ids = self.new_node_ancestor_ids.cloned_ref();

        for ancestor_id in &new_node_ancestor_ids {
            let query = Node::PUSH_TO_DESCENDANT_IDS_QUERY;
            batch.append_statement(query, (self.node.id, self.node.root_id, *ancestor_id))?;
        }

        let update_ancestor_node = UpdateAncestors {
            root_id: self.node.root_id,
            id: self.node.id,
            ancestor_ids: Some(new_node_ancestor_ids),
        };

        batch.append_update(update_ancestor_node)?;

        batch.execute(&self.db_session).await?;

        Ok(())
    }

    async fn add_new_ancestors_to_descendants_statements_and_op(
        &mut self,
    ) -> Result<(), NodecosmosError> {
        let descendant_ids = self.node.descendant_ids.clone().unwrap_or_default();
        let mut new_child_ancestor_ids = self.new_node_ancestor_ids.cloned_ref();

        new_child_ancestor_ids.push(self.node.id);

        let new_child_ancestor_ids_ref = &new_child_ancestor_ids; // create reference here
        let descendants_q = find_node_query!("root_id = ? AND id IN ?");

        let mut descendants = Node::find_iter(
            &self.db_session,
            descendants_q,
            (self.root.id, descendant_ids),
            1000,
        )
        .await?;

        while let Some(descendant) = descendants.next().await {
            if let Ok(descendant) = descendant {
                let mut batch = CharybdisModelBatch::new();

                let current_ancestor_ids = descendant.ancestor_ids.cloned_ref();

                // current reordered node + 1 index
                let split_index = current_ancestor_ids
                    .iter()
                    .position(|id| id == &self.node.id)
                    .map_or(current_ancestor_ids.len(), |i| i + 1);

                // take all existing ancestors bellow the reordered_node
                let preserved_ancestor_ids = current_ancestor_ids
                    .iter()
                    .skip(split_index)
                    .cloned()
                    .collect::<Vec<Uuid>>();

                // take reordered_node ancestors and append preserved_ancestor_ids
                let new_ancestor_ids = new_child_ancestor_ids_ref
                    .iter()
                    .cloned()
                    .chain(preserved_ancestor_ids)
                    .collect::<Vec<Uuid>>();

                let update_ancestor_node = UpdateAncestors {
                    root_id: self.node.root_id,
                    id: descendant.id,
                    ancestor_ids: Some(new_ancestor_ids.clone()),
                };

                batch.append_update(update_ancestor_node)?;

                for ancestor_id in new_ancestor_ids {
                    let query = Node::PUSH_TO_DESCENDANT_IDS_QUERY;
                    batch
                        .append_statement(query, (descendant.id, self.node.root_id, ancestor_id))?;
                }

                batch.execute(&self.db_session).await?;
            }
        }

        Ok(())
    }
}
