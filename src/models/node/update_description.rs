use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::update::BranchUpdate;
use crate::models::branch::Branch;
use crate::models::description_commit::DescriptionCommit;
use crate::models::node::{GetDescriptionBase64Node, GetDescriptionNode, Node, UpdateDescriptionNode};
use crate::models::node_commit::create::NodeChange;
use crate::models::node_commit::NodeCommit;
use crate::models::traits::Branchable;
use crate::services::elastic::ElasticDocument;
use crate::services::elastic::ElasticIndex;
use crate::utils::logger::log_error;
use ammonia::clean;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use charybdis::model::AsNative;
use charybdis::operations::{Find, Insert};
use elasticsearch::Elasticsearch;
use quick_xml::events::Event;
use quick_xml::name::QName;
use quick_xml::Reader;
use scylla::CachingSession;
use serde::de::Unexpected::Option;
use std::borrow::Cow;
use yrs::updates::decoder::Decode;
use yrs::{Doc, GetString, ReadTxn, Transact, TransactionMut, Update, XmlElementRef};

impl UpdateDescriptionNode {
    pub fn sanitize_description(&mut self) {
        if let Some(description) = &self.description {
            self.description = Some(clean(description));
        }
    }

    pub async fn update_elastic_index(&self, elastic_client: &Elasticsearch) {
        if self.is_original() {
            self.update_elastic_document(elastic_client).await;
        }
    }

    pub async fn create_new_version(&self, req_data: &RequestData) {
        let description_version = DescriptionCommit::new(self.description_base64.clone());

        let _ = description_version
            .insert()
            .execute(req_data.db_session())
            .await
            .map_err(|e| {
                log_error(format!("Failed to create new description version: {}", e));
                e
            });

        let changes = vec![NodeChange::Description(description_version.id)];

        let _ = NodeCommit::handle_change(
            req_data.db_session(),
            self.id,
            self.branch_id,
            req_data.current_user_id(),
            &changes,
            true,
        )
        .await
        .map_err(|e| {
            log_error(format!("Failed to create new description version: {}", e));
            e
        });
    }

    pub async fn preserve_for_branch(&self, req_data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_branched() {
            self.as_native().create_branched_if_not_exist(req_data).await;
        }

        Ok(())
    }

    pub async fn update_branch(&self, req_data: &RequestData) {
        if self.is_branched() {
            Branch::update(
                &req_data.db_session(),
                self.branch_id,
                BranchUpdate::EditNodeDescription(self.id),
            )
            .await;
        }
    }
}
