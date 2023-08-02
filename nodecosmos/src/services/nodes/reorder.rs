use crate::errors::NodecosmosError;
use crate::models::node::{Node, UpdateAncestors, UpdateChildIds, UpdateParent};
use actix_web::web::Data;
use charybdis::{CharybdisModelBatch, Deserialize, Find, List, Uuid};
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
    pub batch: CharybdisModelBatch,
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
            batch: CharybdisModelBatch::new(),
            root,
            node,
            new_parent,
            new_node_ancestor_ids: None,
            db_session,
        })
    }

    pub async fn reorder(&mut self) -> Result<(), NodecosmosError> {
        self.add_node_to_parent()?;

        if self.is_parent_changed() {
            self.add_remove_from_old_parent_statements()?;
            self.add_remove_node_from_old_ancestor_statements()?;
            self.add_remove_node_descendants_from_all_descendants_statements()?;

            self.build_new_ancestor_ids();
            self.add_new_parent_statement()?;
            self.add_node_to_ancestor_descendants()?;
            self.add_new_ancestors_to_node_statement().await?;
            self.add_new_ancestors_to_descendants_statements_and_op()
                .await?;
        }

        self.batch.execute(&self.db_session).await?;

        Ok(())
    }

    fn is_parent_changed(&self) -> bool {
        if let Some(current_parent_id) = self.node.parent_id {
            return current_parent_id != self.params.new_parent_id;
        }

        false
    }

    fn build_new_ancestor_ids(&mut self) {
        let new_parent_ancestors = self
            .new_parent
            .ancestor_ids
            .as_ref()
            .cloned()
            .unwrap_or_default();
        let mut new_ancestors = Vec::with_capacity(new_parent_ancestors.len() + 1);

        new_ancestors.extend(new_parent_ancestors);
        new_ancestors.push(self.new_parent.id);

        self.new_node_ancestor_ids = Some(new_ancestors);
    }

    fn add_remove_from_old_parent_statements(&mut self) -> Result<(), NodecosmosError> {
        if let Some(parent_id) = &self.node.parent_id {
            let query = Node::PULL_FROM_CHILD_IDS_QUERY;
            self.batch
                .append_statement(query, (self.node.id, self.node.root_id, parent_id))?;
        }

        Ok(())
    }

    fn add_remove_node_from_old_ancestor_statements(&mut self) -> Result<(), NodecosmosError> {
        let ancestor_ids = self.node.ancestor_ids.as_ref().cloned().unwrap_or_default();

        for ancestor_id in ancestor_ids {
            let query = Node::PULL_FROM_DESCENDANT_IDS_QUERY;
            self.batch
                .append_statement(query, (self.node.id, self.node.root_id, ancestor_id))?;
        }

        Ok(())
    }

    fn add_remove_node_descendants_from_all_descendants_statements(
        &mut self,
    ) -> Result<(), NodecosmosError> {
        let root_descendant_ids = self
            .root
            .descendant_ids
            .as_ref()
            .cloned()
            .unwrap_or_default();
        let node_descendant_ids = self
            .node
            .descendant_ids
            .as_ref()
            .cloned()
            .unwrap_or_default();

        for descendant_id in node_descendant_ids {
            for ancestor_id in &root_descendant_ids {
                let query = Node::PULL_FROM_DESCENDANT_IDS_QUERY;
                self.batch
                    .append_statement(query, (descendant_id, self.node.root_id, ancestor_id))?;
            }
        }

        Ok(())
    }

    fn add_new_parent_statement(&mut self) -> Result<(), NodecosmosError> {
        let update_parent_node = UpdateParent {
            root_id: self.node.root_id,
            id: self.node.id,
            parent_id: Some(self.params.new_parent_id),
        };

        self.batch.append_update(update_parent_node)?;

        Ok(())
    }

    fn add_node_to_parent(&mut self) -> Result<(), NodecosmosError> {
        let mut new_parent_child_ids = self
            .new_parent
            .child_ids
            .as_ref()
            .cloned()
            .unwrap_or_default();

        if !self.is_parent_changed() {
            new_parent_child_ids.retain(|id| id != &self.node.id);
        }

        new_parent_child_ids.insert(self.params.new_sibling_index, self.node.id);

        let update_child_ids_node = UpdateChildIds {
            root_id: self.node.root_id,
            id: self.params.new_parent_id,
            child_ids: Some(new_parent_child_ids),
        };

        self.batch.append_update(update_child_ids_node)?;

        Ok(())
    }

    fn add_node_to_ancestor_descendants(&mut self) -> Result<(), NodecosmosError> {
        let new_node_ancestor_ids = self
            .new_node_ancestor_ids
            .as_ref()
            .cloned()
            .unwrap_or_default();

        for ancestor_id in new_node_ancestor_ids {
            let query = Node::PUSH_TO_DESCENDANT_IDS_QUERY;
            self.batch
                .append_statement(query, (self.node.id, self.node.root_id, ancestor_id))?;
        }

        Ok(())
    }

    async fn add_new_ancestors_to_node_statement(&mut self) -> Result<(), NodecosmosError> {
        let new_node_ancestor_ids = self
            .new_node_ancestor_ids
            .as_ref()
            .cloned()
            .unwrap_or_default();

        let update_ancestor_node = UpdateAncestors {
            root_id: self.node.root_id,
            id: self.node.id,
            ancestor_ids: Some(new_node_ancestor_ids),
        };

        self.batch.append_update(update_ancestor_node)?;

        Ok(())
    }

    // TODO: fix this
    async fn add_new_ancestors_to_descendants_statements_and_op(
        &mut self,
    ) -> Result<(), NodecosmosError> {
        let descendant_ids = self.node.descendant_ids.clone().unwrap_or_default();
        let mut new_child_ancestor_ids = self
            .new_node_ancestor_ids
            .as_ref()
            .cloned()
            .unwrap_or_default();
        new_child_ancestor_ids.push(self.node.id);

        let new_child_ancestor_ids_ref = &new_child_ancestor_ids; // create reference here

        for descendant_id in descendant_ids {
            let mut update_ancestor_node = UpdateAncestors {
                root_id: self.node.root_id,
                id: descendant_id,
                ..Default::default()
            }
            .find_by_primary_key(&self.db_session)
            .await?;

            let current_ancestor_ids = update_ancestor_node
                .ancestor_ids
                .as_ref()
                .cloned()
                .unwrap_or_default();

            let new_ancestor_ids: List<Uuid> = current_ancestor_ids
                .into_iter()
                .take_while(|&ancestor_id| ancestor_id != self.node.id)
                .chain(new_child_ancestor_ids_ref.iter().cloned()) // use reference here
                .collect();

            for ancestor_id in &new_ancestor_ids {
                let query = Node::PUSH_TO_DESCENDANT_IDS_QUERY;
                self.batch
                    .append_statement(query, (descendant_id, self.node.root_id, ancestor_id))?;
            }

            update_ancestor_node.ancestor_ids = Some(new_ancestor_ids); // reuse the existing object

            self.batch.append_update(update_ancestor_node)?;
        }

        Ok(())
    }
}
