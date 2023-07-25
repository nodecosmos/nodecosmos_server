mod migration;
mod schema;
mod session;

use crate::migration::migration_plan::MigrationPlan;
use schema::*;
use scylla::Session;
use session::initialize_session;

use std::fs::read_dir;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::{env, io};

use clap::Parser;

/// Automatic Migration Tool
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Scylla Host
    #[arg(long)]
    host: String,

    /// Keyspace
    #[arg(short, long)]
    keyspace: String,

    #[arg(short, long, default_value = "")]
    user: String,

    #[arg(short, long, default_value = "")]
    password: String,

    #[arg(short, long, default_value_t = 30)]
    timeout: u64,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let project_root = get_project_root().unwrap();

    let session: Session = initialize_session(&args).await;

    let current_db_schema: CurrentDbSchema =
        CurrentDbSchema::new(&session, args.keyspace).await.unwrap();
    let current_code_schema: CurrentCodeSchema = CurrentCodeSchema::new(&project_root);
    let migration_plan = MigrationPlan::new(&current_db_schema, &current_code_schema, &session);

    migration_plan.run().await;

    current_db_schema
        .write_schema_to_json(project_root)
        .await
        .unwrap_or_else(|e| {
            eprintln!("Error writing schema to json: {}", e);
        });
}

pub(crate) fn get_project_root() -> io::Result<PathBuf> {
    let path = env::current_dir()?;
    let path_ancestors = path.as_path().ancestors();

    for p in path_ancestors {
        let has_cargo = read_dir(p)?
            .any(|p| p.unwrap().file_name() == *"Cargo.lock");
        if has_cargo {
            return Ok(PathBuf::from(p));
        }
    }

    Err(io::Error::new(
        ErrorKind::NotFound,
        "Ran out of places to find Cargo.toml",
    ))
}
