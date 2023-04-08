mod session;
mod migration;
mod schema;

use schema::*;
use colored::*;
use scylla::Session;
use session::initialize_session;
use crate::migration::migration_plan::MigrationPlan;

#[tokio::main]
async fn main() {
    let session: Session = initialize_session().await;

    let current_db_schema: CurrentDbSchema = CurrentDbSchema::new(&session).await.unwrap();
    let current_code_schema: CurrentCodeSchema = CurrentCodeSchema::new();

    let current_code_schema_json = serde_json::to_string_pretty(&current_code_schema).unwrap();

    println!("{}", current_code_schema_json);
    println!("{}", current_db_schema.get_current_schema_as_json().await.unwrap());
    
    let migration_plan: MigrationPlan = MigrationPlan::new(&current_db_schema, &current_code_schema);
    
}
