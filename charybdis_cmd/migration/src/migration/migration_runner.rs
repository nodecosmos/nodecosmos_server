use colored::*;
use scylla::prepared_statement::PreparedStatement;
use scylla::Session;
use super::migration::{Migration, MigrationObjectType};
use scylla::statement::{Consistency};
use crate::schema::SchemaObjectTrait;

impl <'a> Migration <'a> {
    async fn execute(&self, cql: &String) {
        let mut prepared: PreparedStatement = self.session.prepare(cql.clone()).await.unwrap();
        prepared.set_consistency(Consistency::All);

        println!("{} {}", "Running CQL:".bright_green(), cql.bright_cyan());

        self.session.execute(&prepared, &[]).await.unwrap_or_else(|err| {
            panic!("Error running '{}': {}", cql, err);
        });
    }

    pub async fn run_first_migration(&self) {
        println!(
            "{} {} {}",
            "Detected first migration run for ".bright_green(),
            self.migration_object_name.bright_yellow(),
            self.migration_obj_type_str().bright_yellow()
        );

        match self.migration_object_type {
            MigrationObjectType::UDT => {
                let cql = format!(
                    "CREATE TYPE IF NOT EXISTS {} ({});",
                    self.migration_object_name,
                    self.current_code_schema.get_cql_fields()
                );

                self.execute(&cql).await;
            },
            MigrationObjectType::Table => {
                let cql = format!(
                    "CREATE TABLE IF NOT EXISTS {} ({} PRIMARY KEY (({}), {}));",
                    self.migration_object_name,
                    self.current_code_schema.get_cql_fields(),
                    self.partition_key,
                    self.clustering_key
                );

                self.execute(&cql).await;
            },
            MigrationObjectType::MaterializedView => {
                let cql = format!(
                    "CREATE MATERIALIZED VIEW IF NOT EXISTS {} AS {}",
                    self.migration_object_name,
                    self.current_code_schema.get_cql_fields(), // TODO: use statement
                );

                self.execute(&cql).await;
            },
        }
    }

    pub async fn run_field_added_migration(&self) {
        println!(
            "{} {} {}",
            "Detected new fields for ".bright_cyan(),
            self.migration_object_name.bright_yellow(),
            self.migration_obj_type_str().bright_yellow()
        );


        let new_fields = self.new_fields().join(", ");

        let cql = format!(
            "ALTER {} {} ADD ({})",
            self.migration_obj_type_str(),
            self.migration_object_name,
            new_fields,
        );

        self.execute(&cql).await;
    }

    pub async fn run_field_removed_migration(&self) {
        println!(
            "{} {} {}",
            "Detected removed fields for ".bright_cyan(),
            self.migration_object_name.bright_yellow(),
            self.migration_obj_type_str().bright_yellow()
        );

        let removed_fields = self.removed_fields().join(", ");

        let cql = format!(
            "ALTER {} {} DROP ({})",
            self.migration_obj_type_str(),
            self.migration_object_name,
            removed_fields,
        );

        self.execute(&cql).await;
    }
}
