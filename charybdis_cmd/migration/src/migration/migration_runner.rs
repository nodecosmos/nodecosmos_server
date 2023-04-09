use colored::*;
use super::migration::{Migration, MigrationObjectType};
use crate::schema::SchemaObjectTrait;
use futures::executor::block_on;
use futures::TryFutureExt;

impl <'a> Migration <'a> {
    fn execute(&self, cql: &String) {
        block_on(async {
            println!("{} {}\n", "Running CQL:".bright_green(), cql.bright_purple());

            let _= self.session.query(cql.clone(), ()).unwrap_or_else(|err| {
                panic!("Error running '{}': {}", cql, err);
            }).await;
        });
    }

    pub(crate) fn run_first_migration(&self) {
        println!(
            "{} {} {}",
            "Detected first migration run for: ".bright_cyan(),
            self.migration_object_name.bright_yellow(),
            self.migration_obj_type_str().bright_magenta()
        );

        match self.migration_object_type {
            MigrationObjectType::UDT => {
                let cql = format!(
                    "CREATE TYPE IF NOT EXISTS {} (\n{}\n);",
                    self.migration_object_name,
                    self.current_code_schema.get_cql_fields()
                );

                self.execute(&cql);
            },
            MigrationObjectType::Table => {

                let clustering_keys = self.current_code_schema.clustering_keys.join(", ");
                let clustering_keys_clause = if clustering_keys.len() > 0 {
                    format!(",{}", clustering_keys)
                } else {
                    "".to_string()
                };

                let cql = format!(
                    "CREATE TABLE IF NOT EXISTS {}\n (\n{}, \nPRIMARY KEY (({}) {})\n);",
                    self.migration_object_name,
                    self.current_code_schema.get_cql_fields(),
                    self.current_code_schema.partition_keys.join(", "),
                    clustering_keys_clause,
                );

                self.execute(&cql);
            },
            MigrationObjectType::MaterializedView => {
                let mut primary_key = self.current_code_schema.partition_keys.clone();
                primary_key.append(&mut self.current_code_schema.clustering_keys.clone());

                let materialized_view_where_clause = format!(
                    "WHERE {}",
                    primary_key
                        .iter()
                        .map(|field| format!("{} IS NOT NULL", field))
                        .collect::<Vec<String>>()
                        .join(" AND ")
                );


                let mv_fields_without_types = self.current_code_schema.fields.iter().map(|(k,_)| k.clone()).collect::<Vec<String>>();

                let materialized_view_select_clause = format!(
                    "SELECT \n{} \nFROM {}\n {}\n",
                    mv_fields_without_types.join(",\n"),
                    self.current_code_schema.base_table.clone(),
                    materialized_view_where_clause
                );

                let primary_key_clause = format!(
                    "PRIMARY KEY ({})",
                    primary_key.join(", ")
                );

                let cql = format!(
                    "CREATE MATERIALIZED VIEW IF NOT EXISTS {}\nAS {} {}",
                    self.migration_object_name,
                    materialized_view_select_clause,
                    primary_key_clause,
                );

                self.execute(&cql);
            },
        }
    }

    pub(crate) fn run_field_added_migration(&self) {
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

        self.execute(&cql);
    }

    pub(crate) fn run_field_removed_migration(&self) {
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

        self.execute(&cql);
    }

    pub(crate) fn run_index_added_migration(&self) {
        println!(
            "{} {} {}",
            "Detected new indexes for ".bright_cyan(),
            self.migration_object_name.bright_yellow(),
            self.migration_obj_type_str().bright_yellow()
        );

        let new_indexes = self.new_secondary_indexes().join(", ");

        let cql = format!(
            "CREATE INDEX IF NOT EXISTS ON {} ({})",
            self.migration_object_name,
            new_indexes,
        );

        self.execute(&cql);
    }

    pub(crate) fn run_index_removed_migration(&self) {
        println!(
            "{} {} {}",
            "Detected removed indexes for ".bright_cyan(),
            self.migration_object_name.bright_yellow(),
            self.migration_obj_type_str().bright_yellow()
        );

        let removed_indexes = self.removed_secondary_indexes().join(", ");

        let cql = format!(
            "DROP INDEX IF EXISTS ON {} ({})",
            self.migration_object_name,
            removed_indexes,
        );

        self.execute(&cql);
    }
}
