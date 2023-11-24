use crate::errors::NodecosmosError;
use crate::models::input_output::Io;
use scylla::CachingSession;

impl Io {
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
