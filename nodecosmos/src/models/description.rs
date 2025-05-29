use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::archived_description::ArchivedDescription;
use crate::models::traits::Branchable;
use crate::models::traits::Clean;
use crate::models::utils::DescriptionYDocParser;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use charybdis::callbacks::Callbacks;
use charybdis::macros::charybdis_model;
use charybdis::operations::{Find, Insert};
use charybdis::types::{Text, Timestamp, Uuid};
use chrono::Utc;
use macros::{Branchable, ObjectId};
use scylla::client::caching_session::CachingSession;
use serde::{Deserialize, Serialize};
use yrs::updates::decoder::Decode;
use yrs::{Doc, Transact, Update};

mod save;
// TODO: fix flow step deletions
#[charybdis_model(
    table_name = description,
    partition_keys = [branch_id],
    clustering_keys = [object_id],
    global_secondary_indexes = []
)]
#[derive(Default, Clone, Branchable, ObjectId, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Description {
    pub node_id: Uuid,
    pub branch_id: Uuid,
    pub object_id: Uuid,

    #[branch(original_id)]
    pub root_id: Uuid,

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
        if self.root_id == Uuid::default() {
            return Err(NodecosmosError::ValidationError(("Root id", "is required")));
        }

        let current = Self::maybe_find_first_by_branch_id_and_object_id(self.branch_id, self.object_id)
            .execute(session)
            .await?;

        if let Some(mut current) = current {
            let branch_id = self.branch_id;

            current.merge(self).await?;

            *self = current;
            self.branch_id = branch_id;
        } else {
            self.update_branch(data).await?;
        }

        self.updated_at = Utc::now();

        self.html.clean()?;

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

    async fn after_delete(
        &mut self,
        db_session: &CachingSession,
        _extension: &Self::Extension,
    ) -> Result<(), Self::Error> {
        let _ = ArchivedDescription::from(&*self)
            .insert()
            .execute(db_session)
            .await
            .map_err(|e| {
                log::error!("[after_delete] Failed to insert archived description: {:?}", e);
                e
            });

        Ok(())
    }
}

impl From<ArchivedDescription> for Description {
    fn from(description: ArchivedDescription) -> Self {
        Self {
            object_id: description.object_id,
            branch_id: description.branch_id,
            node_id: description.node_id,
            root_id: description.root_id,
            object_type: description.object_type,
            short_description: description.short_description,
            html: description.html,
            markdown: description.markdown,
            base64: description.base64,
            updated_at: description.updated_at,
        }
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
        transaction.apply_update(current)?;
        transaction.apply_update(update)?;

        let prose_doc = DescriptionYDocParser::new().run(&transaction, xml)?;
        let html = prose_doc.html;
        let markdown = prose_doc.markdown;
        let short_description = prose_doc.short_description;
        let base64 = STANDARD.encode(transaction.encode_update_v2());

        self.short_description = Some(short_description);
        self.html = Some(html);
        self.markdown = Some(markdown);
        self.base64 = Some(base64);

        Ok(())
    }
}

partial_description!(
    BaseDescription,
    object_id,
    branch_id,
    root_id,
    object_type,
    node_id,
    html,
    markdown,
    short_description
);

impl From<ArchivedDescription> for BaseDescription {
    fn from(description: ArchivedDescription) -> Self {
        Self {
            object_id: description.object_id,
            branch_id: description.branch_id,
            node_id: description.node_id,
            root_id: description.root_id,
            object_type: description.object_type,
            html: description.html,
            markdown: description.markdown,
            short_description: description.short_description,
        }
    }
}

macro_rules! find_branched {
    ($struct_name:ident) => {
        impl $struct_name {
            pub async fn find_branched(&mut self, db_session: &CachingSession) -> Result<&mut Self, NodecosmosError> {
                use anyhow::Context;

                let branch_self = Self::maybe_find_by_primary_key_value((self.branch_id, self.object_id))
                    .execute(db_session)
                    .await
                    .context("Failed to find branched description")?;

                match branch_self {
                    Some(branch_self) => {
                        *self = branch_self;
                    }
                    None => {
                        let branch_id = self.branch_id;

                        if let Some(desc) = Self::maybe_find_by_primary_key_value((self.original_id(), self.object_id))
                            .execute(db_session)
                            .await?
                        {
                            *self = desc;
                        } else if let Some(desc) =
                            ArchivedDescription::maybe_find_by_primary_key_value((self.branch_id, self.object_id))
                                .execute(db_session)
                                .await?
                                .map(|desc| desc.into())
                        {
                            *self = desc;
                        } else {
                            *self =
                                ArchivedDescription::find_by_primary_key_value((self.original_id(), self.object_id))
                                    .execute(db_session)
                                    .await?
                                    .into();
                        }

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
