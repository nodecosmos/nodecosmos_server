use crate::app::App;
use crate::errors::NodecosmosError;
use charybdis::callbacks::Callbacks;
use charybdis::macros::charybdis_model;
use charybdis::types::{Text, Timestamp, Uuid};
use log::error;
use scylla::client::caching_session::CachingSession;
use serde::{Deserialize, Serialize};

#[charybdis_model(
    table_name = contact,
    partition_keys = [id],
    clustering_keys = [],
    global_secondary_indexes = []
)]
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Contact {
    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,

    #[serde(default)]
    pub first_name: Text,

    #[serde(default)]
    pub last_name: Text,

    #[serde(default)]
    pub user_id: Option<Uuid>,

    pub email: Text,

    #[serde(default)]
    pub company_name: Text,

    #[serde(default)]
    pub phone: Text,

    pub message: Text,

    #[serde(default = "chrono::Utc::now")]
    pub created_at: Timestamp,

    #[serde(default = "chrono::Utc::now")]
    pub updated_at: Timestamp,
}

impl Callbacks for Contact {
    type Extension = App;
    type Error = NodecosmosError;

    async fn before_insert(&mut self, _: &CachingSession, app: &App) -> Result<(), Self::Error> {
        let _ = app.mailer.send_contact_us_email(self).await.map_err(|e| {
            error!("Failed to send contact us email: {}.\n Contact: {:#?}", e, &self);

            NodecosmosError::InternalServerError("Failed to send contact us email".to_string())
        });

        Ok(())
    }
}
