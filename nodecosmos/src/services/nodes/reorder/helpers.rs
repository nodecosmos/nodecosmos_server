use crate::errors::NodecosmosError;
use crate::models::helpers::ClonedRef;
use crate::models::node::GetStructureNode;
use charybdis::{Find, Uuid};
use scylla::CachingSession;

pub async fn init_sibling(
    id: Option<Uuid>,
    db_session: &CachingSession,
) -> Result<Option<GetStructureNode>, NodecosmosError> {
    if let Some(id) = id {
        let node = GetStructureNode {
            id,
            ..Default::default()
        }
        .find_by_primary_key(&db_session)
        .await?;

        return Ok(Some(node));
    }

    Ok(None)
}

pub fn build_new_ancestor_ids(new_parent: &GetStructureNode) -> Vec<Uuid> {
    let new_parent_ancestors = new_parent.ancestor_ids.cloned_ref();
    let mut new_ancestors = Vec::with_capacity(new_parent_ancestors.len() + 1);

    new_ancestors.extend(new_parent_ancestors);
    new_ancestors.push(new_parent.id);

    new_ancestors
}

pub fn build_new_index(
    new_upper_sibling: &Option<GetStructureNode>,
    new_bottom_sibling: &Option<GetStructureNode>,
) -> f64 {
    if new_upper_sibling.is_none() && new_bottom_sibling.is_none() {
        return 0.0;
    }

    let upper_sibling_index = if let Some(new_upper_sibling) = new_upper_sibling {
        new_upper_sibling.order_index.unwrap_or_default()
    } else {
        0.0
    };

    let bottom_sibling_index = if let Some(new_bottom_sibling) = new_bottom_sibling {
        new_bottom_sibling.order_index.unwrap_or_default()
    } else {
        0.0
    };

    // If only the bottom sibling exists, return its order index minus 1
    if new_upper_sibling.is_none() {
        return bottom_sibling_index - 1.0;
    }

    // If only the upper sibling exists, return its order index plus 1
    if new_bottom_sibling.is_none() {
        return upper_sibling_index + 1.0;
    }

    // If both siblings exist, return the average of their order indices
    // TODO: This is not ideal, as it has limited precision.
    //  after around of 1000 reorders between 0 and 1 the correct decimal point will be lost.
    //  Let's consider fractional index values in the future
    (upper_sibling_index + bottom_sibling_index) / 2.0
}
