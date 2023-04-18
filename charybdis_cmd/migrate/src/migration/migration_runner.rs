use super::migration::{Migration, MigrationObjectType};
use crate::schema::SchemaObjectTrait;
use colored::*;
use futures::TryFutureExt;
use strip_ansi_escapes::strip;

pub(crate) const INDEX_SUFFIX: &str = "idx";

impl<'a> Migration<'a> {
    async fn execute(&self, cql: &String) {
        println!(
            "{} {}",
            "Running CQL:".on_bright_green().black(),
            cql.bright_purple()
        );

        // remove all colors from cql string
        let stripped = strip(cql.as_bytes()).unwrap();
        let cql: String = String::from_utf8(stripped).unwrap();

        let _ = self
            .session
            .query(cql.clone(), ())
            .unwrap_or_else(|err| {
                panic!(
                    "\n\n{} {}: \n\n{}\n\n",
                    "Error running CQL:".on_red().black(),
                    cql.magenta(),
                    err.to_string().bright_red()
                );
            })
            .await;

        println!("{}\n", "Migration unit completed!".bright_green());
    }

    pub(crate) async fn run_first_migration(&self) {
        println!(
            "\n{} {} {}!",
            "Detected first migration run for:".bright_cyan(),
            self.migration_object_name.bright_yellow(),
            self.migration_obj_type_str().bright_magenta()
        );

        match self.migration_object_type {
            MigrationObjectType::UDT => {
                let cql = format!(
                    "CREATE TYPE IF NOT EXISTS {}\n(\n{}\n);",
                    self.migration_object_name,
                    self.current_code_schema.get_cql_fields()
                );

                self.execute(&cql).await;
            }
            MigrationObjectType::Table => {
                let clustering_keys = self.current_code_schema.clustering_keys.join(", ");
                let clustering_keys_clause = if clustering_keys.len() > 0 {
                    format!(",{}", clustering_keys)
                } else {
                    "".to_string()
                };

                let cql = format!(
                    "CREATE TABLE IF NOT EXISTS {}\n(\n{}, \n    PRIMARY KEY (({}) {})\n);",
                    self.migration_object_name,
                    self.current_code_schema.get_cql_fields(),
                    self.current_code_schema.partition_keys.join(", "),
                    clustering_keys_clause,
                );

                self.execute(&cql).await;
            }
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

                let mv_fields_without_types = self
                    .current_code_schema
                    .fields
                    .iter()
                    .map(|(k, _)| k.clone())
                    .collect::<Vec<String>>();

                let materialized_view_select_clause = format!(
                    "SELECT {} \nFROM {}\n{}\n",
                    mv_fields_without_types.join(", "),
                    self.current_code_schema.base_table.clone(),
                    materialized_view_where_clause
                );

                let primary_key_clause = format!("PRIMARY KEY ({})", primary_key.join(", "));

                let cql = format!(
                    "CREATE MATERIALIZED VIEW IF NOT EXISTS {}\nAS {} {}",
                    self.migration_object_name, materialized_view_select_clause, primary_key_clause,
                );

                self.execute(&cql).await;
            }
        }
    }

    pub(crate) async fn run_field_added_migration(&self) {
        println!(
            "\n{} {} {}",
            "Detected new fields for ".bright_cyan(),
            self.migration_object_name.bright_yellow(),
            self.migration_obj_type_str().bright_yellow()
        );

        if self.migration_object_type == MigrationObjectType::Table {
            self.run_table_field_added_migration().await;
        } else {
            self.run_udt_field_added_migration().await;
        }
    }

    async fn run_table_field_added_migration(&self) {
        let add_fields_clause = self
            .new_fields()
            .iter()
            .map(|(field_name, field_type)| format!("{} {}", field_name, field_type))
            .collect::<Vec<String>>()
            .join(", ");

        let cql = format!(
            "ALTER {} {} ADD ({})",
            self.migration_obj_type_str(),
            self.migration_object_name,
            add_fields_clause,
        );

        self.execute(&cql).await;
    }

    async fn run_udt_field_added_migration(&self) {
        for (field_name, field_type) in self.new_fields() {
            let cql = format!(
                "ALTER TYPE {} ADD {} {}",
                self.migration_object_name, field_name, field_type
            );

            self.execute(&cql).await;
        }
    }

    pub(crate) async fn run_field_removed_migration(&self) {
        println!(
            "\n{} {} {}",
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

    pub(crate) async fn run_index_added_migration(&self) {
        println!(
            "\n{} {} {}",
            "Detected new indexes for ".bright_cyan(),
            self.migration_object_name.bright_yellow(),
            self.migration_obj_type_str().bright_yellow()
        );

        let new_indexes = self.new_secondary_indexes();

        for column_name in new_indexes {
            let index_name: String = self.construct_index_name(&column_name);

            let cql = format!(
                "CREATE INDEX IF NOT EXISTS {} ON {} ({})",
                index_name, self.migration_object_name, column_name,
            );

            self.execute(&cql).await;
        }
    }

    pub(crate) async fn run_index_removed_migration(&self) {
        println!(
            "\n{} {} {}",
            "Detected removed indexes for ".bright_cyan(),
            self.migration_object_name.bright_yellow(),
            self.migration_obj_type_str().bright_yellow()
        );

        let removed_indexes = self.removed_secondary_indexes();

        for index in removed_indexes {
            let cql = format!("DROP INDEX {}", index,);

            self.execute(&cql).await;
        }
    }
}
