use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use charybdis::callbacks::Callbacks;
use charybdis::macros::charybdis_model;
use charybdis::operations::Find;
use charybdis::types::{Text, Timestamp, Uuid};
use chrono::Utc;
use scylla::CachingSession;
use serde::{Deserialize, Serialize};
use yrs::updates::decoder::Decode;
use yrs::{Doc, GetString, Transact, Update};

use nodecosmos_macros::{Branchable, ObjectId};

use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::traits::Branchable;
use crate::models::traits::SanitizeDescription;
use crate::models::utils::DescriptionXmlParser;

mod save;

#[charybdis_model(
    table_name = description,
    partition_keys = [object_id],
    clustering_keys = [branch_id]
)]
#[derive(Default, Clone, Branchable, ObjectId, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Description {
    #[branch(original_id)]
    pub object_id: Uuid,

    pub branch_id: Uuid,
    pub node_id: Uuid,
    pub object_type: Text,
    pub short_description: Option<Text>,
    pub html: Option<Text>,
    pub markdown: Option<Text>,
    pub base64: Option<Text>,

    #[serde(default = "chrono::Utc::now")]
    pub updated_at: Timestamp,
}

impl Callbacks for Description {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_insert(&mut self, session: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        let current = Self::maybe_find_first_by_object_id_and_branch_id(self.object_id, self.branch_id)
            .execute(session)
            .await?;

        if let Some(mut current) = current {
            let branch_id = self.branch_id;

            current.merge(self).await?;

            *self = current;
            self.branch_id = branch_id;
        }

        self.updated_at = Utc::now();

        self.html.sanitize()?;

        self.handle_branch(data).await?;

        Ok(())
    }

    async fn after_insert(&mut self, _: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        let _ = self.update_elastic_index(data).await.map_err(|e| {
            log::error!("[after_insert] Failed to update elastic index: {:?}", e);
            e
        });

        Ok(())
    }

    async fn after_update(&mut self, _: &CachingSession, data: &RequestData) -> Result<(), Self::Error> {
        let _ = self.update_elastic_index(data).await.map_err(|e| {
            log::error!("[after_update] Failed to update elastic index: {:?}", e);
            e
        });

        Ok(())
    }
}

impl Description {
    const DESCRIPTION_ROOT: &'static str = "prosemirror";

    pub async fn merge(&mut self, other: &Self) -> Result<(), NodecosmosError> {
        let current_base64 = match &self.base64 {
            Some(base64) => base64,
            None => {
                self.html = other.html.clone();
                self.markdown = other.markdown.clone();
                self.base64 = other.base64.clone();

                return Ok(());
            }
        };

        let updated_base64 = match &other.base64 {
            Some(base64) => base64,
            None => return Ok(()),
        };

        let current_buf = STANDARD.decode(current_base64)?;
        let update_buf = STANDARD.decode(updated_base64)?;
        let current = Update::decode_v2(&current_buf)?;
        let update = Update::decode_v2(&update_buf)?;
        let doc = Doc::new();
        let xml = doc.get_or_insert_xml_fragment(Self::DESCRIPTION_ROOT);
        let mut transaction = doc.transact_mut();

        transaction.apply_update(current);
        transaction.apply_update(update);

        let xml_str = &xml.get_string(&transaction);
        let prose_doc = DescriptionXmlParser::new(xml_str).run()?;
        let html = prose_doc.html;
        let markdown = prose_doc.markdown;
        let base64 = STANDARD.encode(&transaction.encode_update_v2());
        let short_description = prose_doc.short_description;

        self.short_description = Some(short_description);
        self.html = Some(html);
        self.markdown = Some(markdown);
        self.base64 = Some(base64);

        Ok(())
    }
}

macro_rules! find_branched {
    ($struct_name:ident) => {
        impl $struct_name {
            pub async fn find_branched(&mut self, db_session: &CachingSession) -> Result<&mut Self, NodecosmosError> {
                use anyhow::Context;

                let branch_self = Self::maybe_find_by_primary_key_value(&(self.object_id, self.branch_id))
                    .execute(db_session)
                    .await
                    .context("Failed to find branched description")?;

                match branch_self {
                    Some(branch_self) => {
                        *self = branch_self;
                    }
                    None => {
                        let branch_id = self.branch_id;

                        *self = Self::find_by_primary_key_value(&(self.object_id, self.original_id()))
                            .execute(db_session)
                            .await?;

                        self.branch_id = branch_id;
                    }
                }

                Ok(self)
            }
        }
    };
}

find_branched!(Description);
find_branched!(BaseDescription);

partial_description!(
    BaseDescription,
    object_id,
    branch_id,
    object_type,
    node_id,
    html,
    markdown
);
