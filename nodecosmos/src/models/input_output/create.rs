use crate::errors::NodecosmosError;
use crate::models::input_output::Io;
use charybdis::types::Uuid;
use scylla::CachingSession;
use serde_json::json;

impl Io {
    pub async fn validate_root_node_id(&mut self, session: &CachingSession) -> Result<(), NodecosmosError> {
        let node = self.node(session).await?;
        let root_node_id = node.root_id;

        if self.root_node_id != root_node_id {
            return Err(NodecosmosError::Unauthorized(json!({
                "error": "Unauthorized",
                "message": "Not authorized to add IO for this node!"
            })));
        }

        Ok(())
    }

    pub fn set_defaults(&mut self) {
        let now = chrono::Utc::now();

        self.id = Uuid::new_v4();
        self.created_at = Some(now);
        self.updated_at = Some(now);
    }

    /// We use copy instead of reference, as in future we may add more features
    /// that will require each node within a flow step to have it's own IO.
    pub async fn copy_vals_from_original(&mut self, session: &CachingSession) -> Result<(), NodecosmosError> {
        let original_io = self.original_io(session).await?;

        if let Some(original_io) = original_io {
            self.title = original_io.title;
            self.unit = original_io.unit;
            self.data_type = original_io.data_type;
            self.description = original_io.description;
            self.description_markdown = original_io.description_markdown;
            self.original_id = original_io.original_id;
        } else {
            self.original_id = Some(self.id);
        }

        Ok(())
    }
}
