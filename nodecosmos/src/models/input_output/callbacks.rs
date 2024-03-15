use crate::errors::NodecosmosError;
use crate::models::input_output::{DeleteIo, Io, UpdateDescriptionIo, UpdateTitleIo};
use crate::models::utils::{sanitize_description_cb_fn, updated_at_cb_fn};
use charybdis::callbacks::Callbacks;
use charybdis::model::AsNative;
use scylla::CachingSession;

impl Callbacks for Io {
    type Extension = Option<()>;
    type Error = NodecosmosError;

    async fn before_insert(&mut self, session: &CachingSession, _ext: &Self::Extension) -> Result<(), NodecosmosError> {
        self.validate_root_node_id(session).await?;
        self.set_defaults();
        self.copy_vals_from_original(session).await?;

        Ok(())
    }
}

impl Callbacks for UpdateDescriptionIo {
    type Extension = Option<()>;
    type Error = NodecosmosError;

    sanitize_description_cb_fn!();

    async fn after_update(&mut self, session: &CachingSession, _ext: &Self::Extension) -> Result<(), NodecosmosError> {
        self.update_ios_desc_by_org_id(session).await?;

        Ok(())
    }
}

impl Callbacks for UpdateTitleIo {
    type Extension = Option<()>;
    type Error = NodecosmosError;

    updated_at_cb_fn!();

    async fn after_update(&mut self, session: &CachingSession, _ext: &Self::Extension) -> Result<(), NodecosmosError> {
        self.update_ios_titles_by_org_id(session).await?;

        Ok(())
    }
}

impl Callbacks for DeleteIo {
    type Extension = Option<()>;
    type Error = NodecosmosError;

    async fn before_delete(&mut self, session: &CachingSession, _ext: &Self::Extension) -> Result<(), NodecosmosError> {
        let mut native_io = self.as_native();

        native_io.pull_from_initial_input_ids(session).await?;
        native_io.pull_form_flow_step_outputs(session).await?;
        native_io.pull_from_next_workflow_step(session).await?;

        Ok(())
    }
}
