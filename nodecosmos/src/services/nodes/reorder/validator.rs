use crate::errors::NodecosmosError;
use crate::services::nodes::reorder::Reorderer;
const REORDER_DESCENDANTS_LIMIT: usize = 15000;

pub struct ReorderValidator<'a> {
    pub reorderer: &'a Reorderer<'a>,
}

impl<'a> ReorderValidator<'a> {
    pub fn new(reorderer: &'a Reorderer) -> Self {
        Self { reorderer }
    }

    pub fn validate(&mut self) -> Result<(), NodecosmosError> {
        self.validate_no_root()?;
        self.validate_not_moved_within_self()?;
        self.validate_reorder_limit()?;
        self.validate_no_conflicts()?;

        Ok(())
    }

    fn validate_no_root(&mut self) -> Result<(), NodecosmosError> {
        if let Some(is_root) = self.reorderer.node.is_root {
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
            .reorderer
            .descendant_ids
            .contains(&self.reorderer.params.new_parent_id)
            || self.reorderer.node.id == self.reorderer.params.new_parent_id
        {
            return Err(NodecosmosError::Forbidden(
                "Can not reorder within self".to_string(),
            ));
        }

        Ok(())
    }

    fn validate_reorder_limit(&mut self) -> Result<(), NodecosmosError> {
        if self.reorderer.is_parent_changed()
            && self.reorderer.descendant_ids.len() > REORDER_DESCENDANTS_LIMIT
        {
            return Err(NodecosmosError::Forbidden(format!(
                "Can not reorder more than {} descendants",
                REORDER_DESCENDANTS_LIMIT
            )));
        }

        Ok(())
    }

    fn validate_no_conflicts(&mut self) -> Result<(), NodecosmosError> {
        if let Some(new_upper_sibling) = &self.reorderer.new_upper_sibling {
            if new_upper_sibling.parent_id != Some(self.reorderer.params.new_parent_id) {
                return Err(NodecosmosError::Conflict(
                    "Upper Sibling Moved!".to_string(),
                ));
            }
        }

        if let Some(new_bottom_sibling) = &self.reorderer.new_bottom_sibling {
            if new_bottom_sibling.parent_id != Some(self.reorderer.params.new_parent_id) {
                return Err(NodecosmosError::Conflict(
                    "Bottom Sibling Moved!".to_string(),
                ));
            }

            if let Some(new_upper_sibling) = &self.reorderer.new_upper_sibling {
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
