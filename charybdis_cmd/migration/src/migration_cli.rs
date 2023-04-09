mod session;
mod migration;
mod schema;

use schema::*;
use scylla::Session;
use session::initialize_session;
use crate::migration::migration_plan::MigrationPlan;

#[tokio::main]
async fn main() {
    let session: Session = initialize_session().await;
    let keyspace_name = "nodecosmos".to_string();

    let current_db_schema: CurrentDbSchema = CurrentDbSchema::new(&session, keyspace_name).await.unwrap();
    let current_code_schema: CurrentCodeSchema = CurrentCodeSchema::new();


    let migration_plan = MigrationPlan::new(&current_db_schema, &current_code_schema, &session);
    migration_plan.run();

    current_db_schema.write_schema_to_json().await.unwrap_or_else(|e| {
        eprintln!("Error writing schema to json: {}", e);
    });
}
