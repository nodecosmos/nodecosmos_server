use colored::Colorize;
use scylla::Session;

use crate::migration::migration_runner::INDEX_SUFFIX;
use crate::schema::SchemaObject;

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum MigrationObjectType {
    UDT,
    Table,
    MaterializedView,
}

pub(crate) struct Migration<'a> {
    pub(crate) migration_object_name: &'a String,
    pub(crate) migration_object_type: MigrationObjectType,
    pub(crate) current_code_schema: &'a SchemaObject,
    pub(crate) current_db_schema: &'a SchemaObject,
    pub(crate) is_first_migration_run: bool,
    pub(crate) session: &'a Session,
}

impl<'a> Migration<'a> {
    pub(crate) fn new(
        migration_object_name: &'a String,
        migration_object_type: MigrationObjectType,
        current_code_schema: &'a SchemaObject,
        current_db_schema: &'a SchemaObject,
        session: &'a Session,
    ) -> Self {
        Self {
            migration_object_name,
            migration_object_type,
            current_code_schema,
            is_first_migration_run: current_db_schema.fields.is_empty(),
            current_db_schema,
            session,
        }
    }

    pub(crate) async fn run(&self) {
        if self.is_first_migration_run {
            self.run_first_migration().await;
        }

        if !self.is_first_migration_run && self.field_type_changed() {
            panic!("Field type changed is not supported!");

            // TODO: implement migration flag so on type change
            //  we drop existing column and create new one.
            //  Notify user that this will drop and recreate the column!
            // self.run_field_type_changed_migration();
        }

        if !self.is_first_migration_run && self.migration_object_type != MigrationObjectType::UDT {
            if self.partition_key_changed() {
                panic!(
                    "\n\n{} {} {}\n{}\n\n",
                    "Illegal change in".bright_red(),
                    self.migration_object_name.bright_yellow(),
                    self.migration_obj_type_str().bright_magenta(),
                    "Partition key change is not allowed!".bright_red(),
                );
            }

            if self.clustering_key_changed() {
                panic!(
                    "\n\n{} {} {}\n{}\n\n",
                    "Illegal change in".bright_red(),
                    self.migration_object_name.bright_yellow(),
                    self.migration_obj_type_str().bright_magenta(),
                    "Clustering key change is not allowed!".bright_red(),
                );
            }
        }

        let mut is_any_field_changed = false;

        if !self.is_first_migration_run && self.new_fields().len() > 0 {
            self.panic_on_mv_fields_change();

            self.run_field_added_migration().await;
            is_any_field_changed = true;
        }

        if !self.is_first_migration_run && self.removed_fields().len() > 0 {
            self.panic_on_mv_fields_change();
            self.panic_on_udt_fields_removal();

            self.run_field_removed_migration().await;
            is_any_field_changed = true;
        }

        if self.migration_object_type != MigrationObjectType::UDT {
            if self.new_secondary_indexes().len() > 0 {
                is_any_field_changed = true;
                self.run_index_added_migration().await;
            }

            if !self.is_first_migration_run && self.removed_secondary_indexes().len() > 0 {
                is_any_field_changed = true;
                self.run_index_removed_migration().await;
            }
        }

        if !self.is_first_migration_run && !is_any_field_changed {
            println!(
                "{} {} {}",
                "No changes detected in".bright_green(),
                self.migration_object_name.bright_yellow(),
                self.migration_obj_type_str().bright_magenta()
            );
        }
    }

    pub(crate) fn migration_obj_type_str(&self) -> String {
        match self.migration_object_type {
            MigrationObjectType::UDT => "Type".to_string(),
            MigrationObjectType::Table => "Table".to_string(),
            MigrationObjectType::MaterializedView => "Materialized View".to_string(),
        }
    }

    pub(crate) fn new_fields(&self) -> Vec<(String, String)> {
        let mut new_fields: Vec<(String, String)> = vec![];

        for (field_name, field_type) in self.current_code_schema.fields.iter() {
            if !self.current_db_schema.fields.contains_key(field_name) {
                new_fields.push((field_name.clone(), field_type.clone()));
            }
        }

        new_fields
    }

    pub(crate) fn removed_fields(&self) -> Vec<String> {
        let mut removed_fields: Vec<String> = vec![];

        for (field_name, _) in self.current_db_schema.fields.iter() {
            if !self.current_code_schema.fields.contains_key(field_name) {
                removed_fields.push(field_name.clone());
            }
        }

        removed_fields
    }

    pub(crate) fn new_secondary_indexes(&self) -> Vec<String> {
        let mut new_indexes: Vec<String> = vec![];

        self.current_code_schema
            .secondary_indexes
            .iter()
            .for_each(|sec_index_column| {
                let index_name: String = self.construct_index_name(sec_index_column);

                if !self
                    .current_db_schema
                    .secondary_indexes
                    .contains(&index_name)
                {
                    new_indexes.push(sec_index_column.clone());
                }
            });

        new_indexes
    }

    pub(crate) fn removed_secondary_indexes(&self) -> Vec<String> {
        let mut removed_indexes: Vec<String> = vec![];

        let code_sec_indexes: Vec<String> = self
            .current_code_schema
            .secondary_indexes
            .iter()
            .map(|sec_idx_col| self.construct_index_name(sec_idx_col))
            .collect();

        self.current_db_schema
            .secondary_indexes
            .iter()
            .for_each(|index| {
                if !code_sec_indexes.contains(&index) {
                    removed_indexes.push(index.clone());
                }
            });

        removed_indexes
    }

    pub(crate) fn construct_index_name(&self, column_name: &String) -> String {
        format!(
            "{}_{}_{}",
            self.migration_object_name, column_name, INDEX_SUFFIX
        )
    }

    // private
    fn field_type_changed(&self) -> bool {
        for (field_name, field_type) in self.current_code_schema.fields.iter() {
            if let Some(db_field_type) = self.current_db_schema.fields.get(field_name) {
                let code_field_type = field_type.to_lowercase().replace(" ", "");
                let db_field_type = db_field_type.to_lowercase().replace(" ", "");

                if code_field_type != db_field_type {
                    return true;
                }
            }
        }

        false
    }

    fn panic_on_udt_fields_removal(&self) {
        if self.migration_object_type == MigrationObjectType::UDT {
            if self.removed_fields().len() > 0 {
                panic!(
                    "\n{}\n",
                    "UDT fields removal is not allowed!".bold().bright_red()
                );
            }
        }
    }

    fn panic_on_mv_fields_change(&self) {
        if self.migration_object_type == MigrationObjectType::MaterializedView {
            panic!(
                "\n{}\n",
                "Materialized view fields change is not allowed!"
                    .bold()
                    .bright_red()
            );
        }
    }

    fn partition_key_changed(&self) -> bool {
        let mut code_partition_keys: Vec<String> = self.current_code_schema.partition_keys.clone();
        let mut db_partition_keys = self.current_db_schema.partition_keys.clone();

        code_partition_keys.sort();
        db_partition_keys.sort();

        code_partition_keys != db_partition_keys
    }

    fn clustering_key_changed(&self) -> bool {
        let mut code_clustering_keys = self.current_code_schema.clustering_keys.clone();
        let mut db_clustering_keys = self.current_db_schema.clustering_keys.clone();

        code_clustering_keys.sort();
        db_clustering_keys.sort();

        code_clustering_keys != db_clustering_keys
    }
}
