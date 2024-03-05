use crate::models::traits::ElasticIndex;
use charybdis::model::{AsNative, Model};
use charybdis::types::Uuid;
use colored::Colorize;
use elasticsearch::http::response::Response;
use elasticsearch::{BulkOperation, BulkOperations, BulkParts, DeleteParts, Elasticsearch, IndexParts, UpdateParts};
use log::error;
use serde::Serialize;
use serde_json::json;
use std::fmt::Display;

pub enum ElasticDocumentOp {
    Add,
    Update,
    Delete,
    BulkUpdate,
    BulkDelete,
}

impl Display for ElasticDocumentOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ElasticDocumentOp::Add => write!(f, "Add"),
            ElasticDocumentOp::Update => write!(f, "Update"),
            ElasticDocumentOp::Delete => write!(f, "Delete"),
            ElasticDocumentOp::BulkUpdate => write!(f, "Bulk Update"),
            ElasticDocumentOp::BulkDelete => write!(f, "Bulk Delete"),
        }
    }
}

pub trait ElasticDocument<T: Model + ElasticIndex + Serialize> {
    async fn bulk_update_elastic_documents(client: &Elasticsearch, models: Vec<T>);
    async fn bulk_delete_elastic_documents(client: &Elasticsearch, ids: &Vec<Uuid>);

    async fn handle_response_error(response: Response, op: ElasticDocumentOp) {
        if !response.status_code().is_success() {
            error!(
                "\n{} {} {} \n{} {} \n{} {}\n",
                "Failed to".bright_red().bold(),
                op.to_string().bright_yellow(),
                T::ELASTIC_IDX_NAME.bright_yellow(),
                "Status:".bright_red().bold(),
                response.status_code(),
                "Response body:".bright_red(),
                response.text().await.unwrap_or("No Body!".to_string()).red(),
            );
        }
    }

    async fn add_elastic_document(&self, client: &Elasticsearch);
    async fn update_elastic_document(&self, client: &Elasticsearch);
    async fn delete_elastic_document(&self, client: &Elasticsearch);
}

impl<T: Model + ElasticIndex + Serialize> ElasticDocument<T> for T {
    async fn bulk_update_elastic_documents(client: &Elasticsearch, models: Vec<T>) {
        let mut ops = BulkOperations::new();

        for model in models {
            let index_id = model.index_id();
            let op = BulkOperation::update(
                index_id.clone(),
                json!({
                    "doc": model
                }),
            );
            let _ = ops.push(op).map_err(|_| {
                error!(
                    "Failed to add update operation to bulk request! Index: {}, Id: {}",
                    T::ELASTIC_IDX_NAME,
                    index_id
                )
            });
        }

        let bulk_response = client
            .bulk(BulkParts::Index(T::ELASTIC_IDX_NAME))
            .body(vec![ops])
            .send()
            .await;

        match bulk_response {
            Ok(bulk_response) => Self::handle_response_error(bulk_response, ElasticDocumentOp::BulkUpdate).await,
            Err(e) => {
                error!(
                    "Failed to send bulk update request! Index: {}, \nResponse: {:?}",
                    T::ELASTIC_IDX_NAME,
                    e
                );
            }
        }
    }

    async fn bulk_delete_elastic_documents(client: &Elasticsearch, ids: &Vec<Uuid>) {
        let mut ops = BulkOperations::new();

        for id in ids {
            ops.push(BulkOperation::<()>::delete(id.to_string()))
                .unwrap_or_else(|_| {
                    error!(
                        "Failed to add delete operation to bulk request! Index: {}, Id: {}",
                        T::ELASTIC_IDX_NAME,
                        id
                    )
                });
        }

        let bulk_response = client
            .bulk(BulkParts::Index(T::ELASTIC_IDX_NAME))
            .body(vec![ops])
            .send()
            .await;

        match bulk_response {
            Ok(bulk_response) => Self::handle_response_error(bulk_response, ElasticDocumentOp::BulkDelete).await,
            Err(e) => {
                error!(
                    "Failed to send bulk delete request! Index: {}, \nResponse: {:?}",
                    T::ELASTIC_IDX_NAME,
                    e
                );
            }
        }
    }

    async fn add_elastic_document(&self, client: &Elasticsearch) {
        let response = client
            .index(IndexParts::IndexId(T::ELASTIC_IDX_NAME, &self.index_id()))
            .body(&self)
            .send()
            .await;

        match response {
            Ok(response) => Self::handle_response_error(response, ElasticDocumentOp::Add).await,
            Err(e) => {
                error!(
                    "Failed to send add request! Index: {}, \nResponse: {:?}",
                    T::ELASTIC_IDX_NAME,
                    e
                );
            }
        }
    }

    async fn update_elastic_document(&self, client: &Elasticsearch) {
        let response = client
            .update(UpdateParts::IndexId(T::ELASTIC_IDX_NAME, &self.index_id()))
            .body(json!({
                "doc": self
            }))
            .send()
            .await;

        match response {
            Ok(response) => Self::handle_response_error(response, ElasticDocumentOp::Update).await,
            Err(e) => {
                error!(
                    "Failed to send update request! Index: {}, \nResponse: {:?}",
                    T::ELASTIC_IDX_NAME,
                    e
                );
            }
        }
    }

    async fn delete_elastic_document(&self, client: &Elasticsearch) {
        let response = client
            .delete(DeleteParts::IndexId(T::ELASTIC_IDX_NAME, &self.index_id()))
            .send()
            .await;

        match response {
            Ok(response) => Self::handle_response_error(response, ElasticDocumentOp::Delete).await,
            Err(e) => {
                error!(
                    "Failed to send delete request! Index: {}, \nResponse: {:?}",
                    T::ELASTIC_IDX_NAME,
                    e
                );
            }
        }
    }
}
