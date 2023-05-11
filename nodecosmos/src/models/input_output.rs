use crate::models::input_output_template::InputOutputTemplate;
use charybdis::*;
use chrono::Utc;

#[derive(Clone)]
#[partial_model_generator]
#[charybdis_model(
    table_name = "input_outputs",
    partition_keys = ["workflow_id"],
    clustering_keys = ["id"],
    secondary_indexes = []
)]
pub struct InputOutput {
    pub workflow_id: Uuid,
    pub id: Uuid,

    pub title: Text,
    pub unit: Text,

    #[serde(rename = "dataType")]
    pub data_type: Text,

    pub value: Text,

    #[serde(rename = "createdAt")]
    pub created_at: Option<Timestamp>,

    #[serde(rename = "updatedAt")]
    pub updated_at: Option<Timestamp>,
}

impl Callbacks for InputOutput {
    async fn before_insert(&mut self, _session: &CachingSession) -> Result<(), CharybdisError> {
        let now = Utc::now();

        self.id = Uuid::new_v4();
        self.created_at = Some(now);
        self.updated_at = Some(now);

        Ok(())
    }

    async fn after_insert(&mut self, session: &CachingSession) -> Result<(), CharybdisError> {
        // TODO: handle this async once we build logic
        //   currently we can not spawn as CachingSession does not implement Clone
        InputOutputTemplate::insert_new_io_temp(session, self.clone()).await;

        Ok(())
    }

    async fn before_update(&mut self, _session: &CachingSession) -> Result<(), CharybdisError> {
        let now = Utc::now();

        self.updated_at = Some(now);

        Ok(())
    }

    async fn after_delete(&mut self, session: &CachingSession) -> Result<(), CharybdisError> {
        InputOutputTemplate::remove_io_temp_if_not_used(session, self.clone()).await;

        Ok(())
    }
}
