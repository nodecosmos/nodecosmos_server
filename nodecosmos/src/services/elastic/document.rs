use charybdis::model::Model;
use charybdis::types::Uuid;
use colored::Colorize;
use elasticsearch::{
    BulkOperation, BulkOperations, BulkParts, DeleteParts, Elasticsearch, IndexParts, UpdateParts,
};
use serde::Serialize;
use serde_json::json;

pub async fn add_elastic_document<T: Model + Serialize>(
    client: &Elasticsearch,
    index: &str,
    model: &T,
    id: String,
) {
    let response = client
        .index(IndexParts::IndexId(index, &id))
        .body(&model)
        .send()
        .await;

    if let Ok(response) = response {
        if !response.status_code().is_success() {
            println!(
                "\n{} {} \n{} {} \n{} {}\n{} {:?}\n",
                "Failed to index".bright_red().bold(),
                index.bright_yellow(),
                "Status:".bright_red().bold(),
                response.status_code(),
                "Response body:".bright_red(),
                response
                    .text()
                    .await
                    .unwrap_or("No Body!".to_string())
                    .red(),
                "Model:".bright_red().bold(),
                serde_json::to_string(&model).unwrap_or("No Model!".to_string())
            );
        }
    } else {
        println!(
            "Failed to send index request! Index: {}, \nResponse: {:?}",
            index, response
        );
    }
}

pub async fn update_elastic_document<T: Model + Serialize>(
    client: &Elasticsearch,
    index: &str,
    model: &T,
    id: String,
) {
    let response = client
        .update(UpdateParts::IndexId(index, &id))
        .body(json!({
            "doc": model
        }))
        .send()
        .await;

    if let Ok(response) = response {
        if !response.status_code().is_success() {
            println!(
                "\n{} {} \n{} {} \n{} {}\n \n{} {}\n",
                "Failed to update".bright_red().bold(),
                index.bright_yellow(),
                "Status:".bright_red().bold(),
                response.status_code(),
                "Response body:".bright_red(),
                response
                    .text()
                    .await
                    .unwrap_or("No Body!".to_string())
                    .red(),
                "Model:".bright_red().bold(),
                serde_json::to_string(&model).unwrap_or("No Model!".to_string())
            );
        }
    } else {
        println!(
            "Failed to send update request! Index: {}, \nResponse: {:?}",
            index, response
        );
    }
}

pub async fn delete_elastic_document(client: &Elasticsearch, index: &str, id: String) {
    let response = client.delete(DeleteParts::IndexId(index, &id)).send().await;

    if let Ok(response) = response {
        if !response.status_code().is_success() {
            println!(
                "\n{} {} \n{} {} \n{} {}\n",
                "Failed to remove".bright_red().bold(),
                index.bright_yellow(),
                "Status:".bright_red().bold(),
                response.status_code(),
                "Response body:".bright_red(),
                response
                    .text()
                    .await
                    .unwrap_or("No Body!".to_string())
                    .red(),
            );
        }
    } else {
        println!(
            "Failed to send remove request! Index: {}, \nResponse: {:?}",
            index, response
        );
    }
}

pub async fn bulk_delete_elastic_documents(client: &Elasticsearch, index: &str, ids: &Vec<Uuid>) {
    let mut ops = BulkOperations::new();

    for id in ids {
        ops.push(BulkOperation::<()>::delete(id.to_string()))
            .unwrap_or_else(|_| {
                println!(
                    "Failed to add delete operation to bulk request! Index: {}, Id: {}",
                    index, id
                )
            });
    }

    let bulk_response = client
        .bulk(BulkParts::Index(index))
        .body(vec![ops])
        .send()
        .await;

    if let Ok(bulk_response) = bulk_response {
        if !bulk_response.status_code().is_success() {
            println!(
                "\n{} {} \n{} {} \n{} {}\n",
                "Failed to bulk delete".bright_red().bold(),
                index.bright_yellow(),
                "Status:".bright_red().bold(),
                bulk_response.status_code(),
                "Response body:".bright_red(),
                bulk_response
                    .text()
                    .await
                    .unwrap_or("No Body!".to_string())
                    .red(),
            );
        }
    } else {
        println!(
            "Failed to send bulk delete request! Index: {}, \nResponse: {:?}",
            index, bulk_response
        );
    }
}
