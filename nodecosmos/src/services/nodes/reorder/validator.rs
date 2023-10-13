use crate::errors::NodecosmosError;
use crate::services::nodes::reorder::reorder_data::ReorderData;
const TREE_DESCENDANTS_LIMIT: usize = 50000; //  5mb of node_descendants data loaded into memory
const REORDER_DESCENDANTS_LIMIT: usize = 15000;

pub struct ReorderValidator<'a> {
    pub reorder_data: &'a ReorderData,
}

impl<'a> ReorderValidator<'a> {
    pub fn new(reorder_data: &'a ReorderData) -> Self {
        Self { reorder_data }
    }

    pub fn validate(&mut self) -> Result<(), NodecosmosError> {
        self.validate_no_root()?;
        self.validate_not_moved_within_self()?;
        self.validate_reorder_limit()?;
        self.validate_no_conflicts()?;

        Ok(())
    }

    fn validate_no_root(&mut self) -> Result<(), NodecosmosError> {
        if let Some(is_root) = self.reorder_data.node.is_root {
            if is_root {
                return Err(NodecosmosError::Forbidden(
                    "Reorder is not allowed for root nodes".to_string(),
                ));
            }
        }

        Ok(())
    }

    fn validate_not_moved_within_self(&mut self) -> Result<(), NodecosmosError> {
        if self
            .reorder_data
            .descendant_ids
            .contains(&self.reorder_data.new_parent.id)
            || self.reorder_data.node.id == self.reorder_data.new_parent.id
        {
            return Err(NodecosmosError::Forbidden(
                "Can not reorder within self".to_string(),
            ));
        }

        Ok(())
    }

    fn validate_reorder_limit(&mut self) -> Result<(), NodecosmosError> {
        if self.reorder_data.is_parent_changed()
            && self.reorder_data.descendant_ids.len() > REORDER_DESCENDANTS_LIMIT
        {
            return Err(NodecosmosError::Forbidden(format!(
                "Can not reorder more than {} descendants",
                REORDER_DESCENDANTS_LIMIT
            )));
        }

        if self.reorder_data.tree_descendants.len() > TREE_DESCENDANTS_LIMIT {
            return Err(NodecosmosError::Forbidden("Tree too large".to_string()));
        }

        Ok(())
    }

    fn validate_no_conflicts(&mut self) -> Result<(), NodecosmosError> {
        if let Some(new_upper_sibling) = &self.reorder_data.new_upper_sibling {
            if new_upper_sibling.parent_id != Some(self.reorder_data.new_parent.id) {
                return Err(NodecosmosError::Conflict(
                    "Upper Sibling Moved!".to_string(),
                ));
            }
        }

        if let Some(new_bottom_sibling) = &self.reorder_data.new_bottom_sibling {
            if new_bottom_sibling.parent_id != Some(self.reorder_data.new_parent.id) {
                return Err(NodecosmosError::Conflict(
                    "Bottom Sibling Moved!".to_string(),
                ));
            }

            if let Some(new_upper_sibling) = &self.reorder_data.new_upper_sibling {
                let new_bottom_sibling_index = new_bottom_sibling.order_index.unwrap_or_default();
                let new_upper_sibling_index = new_upper_sibling.order_index.unwrap_or_default();

                if new_bottom_sibling_index < new_upper_sibling_index {
                    return Err(NodecosmosError::Conflict("Siblings Reordered!".to_string()));
                }
            }
        }

        Ok(())
    }
}
