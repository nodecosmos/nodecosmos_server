use crate::models::helpers::updated_at_cb_fn;
use crate::models::input_output::InputOutput;
use charybdis::*;
use chrono::Utc;

// TODO: to-be used for suggesting IOs
#[partial_model_generator]
#[charybdis_model(
    table_name = input_output_templates,
    partition_keys = [title],
    clustering_keys = [unit],
    secondary_indexes = []
)]
pub struct InputOutputTemplate {
    pub title: Text,
    pub unit: Text,

    #[serde(rename = "dataType")]
    pub data_type: Text,

    #[serde(rename = "createdAt")]
    pub created_at: Option<Timestamp>,

    #[serde(rename = "updatedAt")]
    pub updated_at: Option<Timestamp>,

    #[serde(rename = "usedByWorkflowIds")]
    pub used_by_workflow_ids: Option<List<Uuid>>,
}

impl InputOutputTemplate {
    pub async fn insert_new_io_temp(session: &CachingSession, io: InputOutput) {
        let mut new_io = InputOutputTemplate {
            title: io.title.clone(),
            unit: io.unit.clone(),
            data_type: io.data_type.clone(),
            created_at: None,
            updated_at: None,
            used_by_workflow_ids: Some(vec![io.workflow_id.clone()] as List<Uuid>),
        };

        let existing_io = new_io.find_by_primary_key(session).await.ok();

        if let Some(existing_io) = existing_io {
            let query = InputOutputTemplate::PUSH_TO_USED_BY_WORKFLOW_IDS_QUERY;
            let _ = execute(
                session,
                query,
                (io.workflow_id, existing_io.title, existing_io.unit),
            )
            .await
            .map_err(|e| println!("Error pushing to used_by_workflow_step_ids: {:?}", e));
        } else {
            let _ = new_io
                .insert_cb(session)
                .await
                .map_err(|e| println!("Error inserting new IO: {:?}", e));
        }
    }

    pub async fn remove_io_temp_if_not_used(session: &CachingSession, io: InputOutput) {
        let new_io = InputOutputTemplate {
            title: io.title.clone(),
            unit: io.unit.clone(),
            ..Default::default()
        };

        let existing_io = &mut new_io.find_by_primary_key(session).await.ok();

        if let Some(ref mut existing_io) = existing_io {
            if let Some(used_by_workflow_step_ids) = &existing_io.used_by_workflow_ids {
                if used_by_workflow_step_ids.len() > 1 {
                    let query = InputOutputTemplate::PULL_FROM_USED_BY_WORKFLOW_IDS_QUERY;
                    let _ = execute(
                        session,
                        query,
                        (io.workflow_id, &existing_io.title, &existing_io.unit),
                    )
                    .await
                    .map_err(|e| {
                        println!("Error removing from used_by_workflow_step_ids: {:?}", e)
                    });

                    return;
                }
            }

            existing_io.delete_cb(session).await.ok();
        } else {
            println!("IO not found");
        }
    }
}

impl Callbacks for InputOutputTemplate {
    async fn before_insert(&mut self, _session: &CachingSession) -> Result<(), CharybdisError> {
        let now = Utc::now();

        self.created_at = Some(now);
        self.updated_at = Some(now);

        Ok(())
    }

    updated_at_cb_fn!();
}
