use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::node::reorder::ReorderParams;
use crate::models::node::{BaseNode, GetStructureNode, Node};
use crate::models::node_descendant::NodeDescendant;
use crate::models::traits::cloned_ref::ClonedRef;
use crate::models::traits::node::{Descendants, FindBranched, Parent};
use crate::models::traits::{Branchable, Pluck};
use charybdis::operations::Find;
use charybdis::types::{Set, Uuid};
use scylla::statement::Consistency;
use scylla::CachingSession;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct ReorderData {
    pub node: Node,
    pub branch_id: Uuid,

    pub descendants: Vec<NodeDescendant>,
    pub descendant_ids: Vec<Uuid>,

    pub old_parent_id: Option<Uuid>,
    pub new_parent: BaseNode,

    pub old_ancestor_ids: Set<Uuid>,
    pub new_ancestor_ids: Set<Uuid>,

    pub removed_ancestor_ids: Vec<Uuid>,
    pub added_ancestor_ids: Vec<Uuid>,

    pub new_upper_sibling_id: Option<Uuid>,
    pub new_lower_sibling_id: Option<Uuid>,

    pub new_upper_sibling: Option<GetStructureNode>,
    pub new_lower_sibling: Option<GetStructureNode>,

    pub old_order_index: f64,
    pub new_order_index: f64,

    pub tree_root: GetStructureNode,
    pub tree_descendants: Vec<NodeDescendant>,
}

impl ReorderData {
    async fn find_descendants(
        db_session: &CachingSession,
        params: &ReorderParams,
        node: &Node,
    ) -> Result<Vec<NodeDescendant>, NodecosmosError> {
        return if params.is_branched() {
            node.branch_descendants(&db_session, Some(Consistency::Quorum)).await
        } else {
            node.descendants(&db_session, Some(Consistency::Quorum))
                .await?
                .try_collect()
                .await
                .map_err(NodecosmosError::from)
        };
    }

    async fn find_new_parent(db_session: &CachingSession, params: &ReorderParams) -> Result<BaseNode, NodecosmosError> {
        let mut query_new_parent_node = Node {
            id: params.id,
            branch_id: params.branch_id,
            parent_id: Some(params.new_parent_id),
            ..Default::default()
        };
        let new_parent = query_new_parent_node.branch_parent(&db_session).await?;

        match new_parent {
            Some(new_parent) => Ok(new_parent.clone()),
            None => Err(NodecosmosError::NotFound(format!(
                "Parent with id {} and branch_id {} not found",
                params.new_parent_id, params.branch_id
            ))),
        }
    }

    async fn find_tree_root(db_session: &CachingSession, node: &Node) -> Result<GetStructureNode, NodecosmosError> {
        let tree_root = GetStructureNode {
            id: node.root_id,
            branch_id: node.branchise_id(node.root_id),
            ..Default::default()
        }
        .find_by_primary_key()
        .execute(&db_session)
        .await?;

        Ok(tree_root)
    }

    async fn find_tree_descendants(
        db_session: &CachingSession,
        tree_root: &GetStructureNode,
        params: &ReorderParams,
    ) -> Result<Vec<NodeDescendant>, NodecosmosError> {
        if params.is_branched() {
            tree_root
                .branch_descendants(&db_session, Some(Consistency::Quorum))
                .await
        } else {
            tree_root
                .descendants(&db_session, Some(Consistency::Quorum))
                .await?
                .try_collect()
                .await
                .map_err(NodecosmosError::from)
        }
    }

    async fn init_sibling(
        db_session: &CachingSession,
        id: Option<Uuid>,
        branch_id: Uuid,
    ) -> Result<Option<GetStructureNode>, NodecosmosError> {
        if let Some(id) = id {
            let node =
                GetStructureNode::find_branched_or_original(&db_session, id, branch_id, Some(Consistency::Quorum))
                    .await?;

            return Ok(Some(node));
        }

        Ok(None)
    }

    fn build_new_ancestor_ids(new_parent: &BaseNode) -> Set<Uuid> {
        let mut new_ancestors = new_parent.ancestor_ids.cloned_ref();

        new_ancestors.insert(new_parent.id);

        new_ancestors
    }

    fn build_new_index(
        new_upper_sibling: &Option<GetStructureNode>,
        new_lower_sibling: &Option<GetStructureNode>,
    ) -> f64 {
        if new_upper_sibling.is_none() && new_lower_sibling.is_none() {
            return 0.0;
        }

        let upper_sibling_index = if let Some(new_upper_sibling) = new_upper_sibling {
            new_upper_sibling.order_index
        } else {
            0.0
        };

        let lower_sibling_index = if let Some(new_lower_sibling) = new_lower_sibling {
            new_lower_sibling.order_index
        } else {
            0.0
        };

        // If only the bottom sibling exists, return its order index minus 1
        if new_upper_sibling.is_none() {
            return lower_sibling_index - 1.0;
        }

        // If only the upper sibling exists, return its order index plus 1
        if new_lower_sibling.is_none() {
            return upper_sibling_index + 1.0;
        }

        // If both siblings exist, return the average of their order indices
        // TODO: This is not ideal, as it has limited precision.
        (upper_sibling_index + lower_sibling_index) / 2.0
    }

    fn extract_removed_ancestor_ids(old_ancestor_ids: &Set<Uuid>, new_ancestor_ids: &Set<Uuid>) -> Vec<Uuid> {
        old_ancestor_ids
            .iter()
            .filter(|&id| !new_ancestor_ids.contains(id))
            .cloned()
            .collect()
    }

    fn extract_added_ancestor_ids(old_ancestor_ids: &Set<Uuid>, new_ancestor_ids: &Set<Uuid>) -> Vec<Uuid> {
        new_ancestor_ids
            .iter()
            .filter(|&id| !old_ancestor_ids.contains(id))
            .cloned()
            .collect()
    }

    pub async fn from_params(params: &ReorderParams, data: &RequestData) -> Result<Self, NodecosmosError> {
        let node = Node::find_or_insert_branched(data, params.id, params.branch_id, Some(Consistency::Quorum)).await?;
        let descendants = Self::find_descendants(data.db_session(), params, &node).await?;
        let descendant_ids = descendants.pluck_id();

        let old_parent_id = node.parent_id;
        let new_parent = Self::find_new_parent(data.db_session(), params).await?;

        let old_ancestor_ids = node.ancestor_ids.cloned_ref();
        let new_ancestor_ids = Self::build_new_ancestor_ids(&new_parent);

        let removed_ancestor_ids = Self::extract_removed_ancestor_ids(&old_ancestor_ids, &new_ancestor_ids);
        let added_ancestor_ids = Self::extract_added_ancestor_ids(&old_ancestor_ids, &new_ancestor_ids);

        let new_upper_sibling =
            Self::init_sibling(data.db_session(), params.new_upper_sibling_id, params.branch_id).await?;
        let new_lower_sibling =
            Self::init_sibling(data.db_session(), params.new_lower_sibling_id, params.branch_id).await?;

        let old_order_index = node.order_index;
        let new_order_index = match params.new_order_index {
            Some(index) => index,
            None => Self::build_new_index(&new_upper_sibling, &new_lower_sibling),
        };

        // used for recovery in case of failure mid-reorder
        let tree_root = Self::find_tree_root(data.db_session(), &node).await?;
        let tree_descendants = Self::find_tree_descendants(data.db_session(), &tree_root, params).await?;

        let data = ReorderData {
            node,
            branch_id: params.branch_id,
            descendants,
            descendant_ids,

            old_parent_id,
            new_parent,

            old_ancestor_ids,
            new_ancestor_ids,

            removed_ancestor_ids,
            added_ancestor_ids,

            new_upper_sibling_id: params.new_upper_sibling_id,
            new_lower_sibling_id: params.new_lower_sibling_id,

            new_upper_sibling,
            new_lower_sibling,

            old_order_index,
            new_order_index,

            tree_root,
            tree_descendants,
        };

        Ok(data)
    }

    pub fn parent_changed(&self) -> bool {
        if let Some(current_parent_id) = self.node.parent_id {
            return current_parent_id != self.new_parent.id;
        }

        false
    }
}
