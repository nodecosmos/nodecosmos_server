use crate::errors::NodecosmosError;
use crate::models::input_output::{DeleteIo, Io, UpdateDescriptionIo, UpdateTitleIo};
use crate::models::utils::{sanitize_description_cb_fn, updated_at_cb_fn};
use charybdis::callbacks::Callbacks;
use charybdis::model::AsNative;
use charybdis::types::Uuid;
use scylla::CachingSession;

impl Callbacks<NodecosmosError> for Io {
    async fn before_insert(&mut self, session: &CachingSession) -> Result<(), NodecosmosError> {
        let now = chrono::Utc::now();

        self.id = Uuid::new_v4();
        self.created_at = Some(now);
        self.updated_at = Some(now);

        self.copy_vals_from_original(session).await?;

        Ok(())
    }
}

impl Callbacks<NodecosmosError> for UpdateDescriptionIo {
    sanitize_description_cb_fn!();

    async fn after_update(&mut self, session: &CachingSession) -> Result<(), NodecosmosError> {
        self.update_ios_desc_by_org_id(session).await?;

        Ok(())
    }
}

impl Callbacks<NodecosmosError> for UpdateTitleIo {
    updated_at_cb_fn!();

    async fn after_update(&mut self, session: &CachingSession) -> Result<(), NodecosmosError> {
        self.update_ios_titles_by_org_id(session).await?;

        Ok(())
    }
}

impl Callbacks<NodecosmosError> for DeleteIo {
    async fn before_delete(&mut self, session: &CachingSession) -> Result<(), NodecosmosError> {
        let mut native_io = self.as_native();

        native_io.pull_from_initial_input_ids(session).await?;
        native_io.pull_form_flow_step_outputs(session).await?;
        native_io.pull_from_next_workflow_step(session).await?;

        Ok(())
    }
}
