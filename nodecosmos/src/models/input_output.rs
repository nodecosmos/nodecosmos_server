use crate::models::helpers::{created_at_cb_fn, updated_at_cb_fn};
use crate::models::input_output_template::InputOutputTemplate;
use charybdis::*;
use chrono::Utc;

#[derive(Clone)]
#[partial_model_generator]
#[charybdis_model(
    table_name = input_outputs,
    partition_keys = [workflow_id],
    clustering_keys = [id],
    secondary_indexes = []
)]
pub struct InputOutput {
    #[serde(rename = "workflowId")]
    pub workflow_id: Uuid,

    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,

    pub title: Option<Text>,
    pub unit: Option<Text>,

    #[serde(rename = "dataType")]
    pub data_type: Option<Text>,

    pub value: Option<Text>,

    #[serde(rename = "createdAt")]
    pub created_at: Option<Timestamp>,

    #[serde(rename = "updatedAt")]
    pub updated_at: Option<Timestamp>,
}

impl Callbacks for InputOutput {
    created_at_cb_fn!();

    async fn after_insert(&mut self, session: &CachingSession) -> Result<(), CharybdisError> {
        // TODO: handle this async once we build logic
        //   currently we can not spawn as CachingSession does not implement Clone
        InputOutputTemplate::insert_new_io_temp(session, self.clone()).await;

        Ok(())
    }

    updated_at_cb_fn!();

    async fn after_delete(&mut self, session: &CachingSession) -> Result<(), CharybdisError> {
        InputOutputTemplate::remove_io_temp_if_not_used(session, self.clone()).await;

        Ok(())
    }
}
