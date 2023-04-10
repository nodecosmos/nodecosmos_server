use colored::Colorize;
use scylla::Session;

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

impl <'a> Migration<'a>  {
    pub(crate) fn new(migration_object_name: &'a String,
               migration_object_type: MigrationObjectType,
               current_code_schema: &'a SchemaObject,
               current_db_schema: &'a SchemaObject,
               session: &'a Session) -> Self {
        Self {
            migration_object_name,
            migration_object_type,
            current_code_schema,
            is_first_migration_run: current_db_schema.fields.is_empty(),
            current_db_schema,
            session,
        }
    }

    pub(crate) async fn run (&self) {
        if self.is_first_migration_run {
            self.run_first_migration().await;
            return;
        }

        if self.field_type_changed() {
            panic!("Field type changed is not supported yet!");
            // TODO: implement question to user if he wants to continue
            //  notify user that this will drop the column and create it again

            // self.run_field_type_changed_migration();
        }

        if self.migration_object_type != MigrationObjectType::UDT {
            if self.partition_key_changed() { panic!("Partition key change is not allowed!"); }
            if self.clustering_key_changed() { panic!("Clustering key change is not allowed!"); }
        }

        let mut is_any_field_changed = false;

        if self.new_fields().len() > 0 {
            self.panic_on_mv_fields_change();
            self.run_field_added_migration().await;
            is_any_field_changed = true;
        }

        if self.removed_fields().len() > 0 {
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

            if self.removed_secondary_indexes().len() > 0 {
                is_any_field_changed = true;
                self.run_index_removed_migration().await;
            }
        }

        if !is_any_field_changed {
            println!(
                "{} {} {}",
                "No changes detected for ".bright_green(),
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

        self.current_code_schema.secondary_indexes.iter().for_each(|index| {
            if !self.current_db_schema.secondary_indexes.contains(index) {
                new_indexes.push(index.clone());
            }
        });

        new_indexes
    }

    pub(crate) fn removed_secondary_indexes(&self) -> Vec<String> {
        let mut removed_indexes: Vec<String> = vec![];

        self.current_db_schema.secondary_indexes.iter().for_each(|index| {
            if !self.current_code_schema.secondary_indexes.contains(index) {
                removed_indexes.push(index.clone());
            }
        });

        removed_indexes
    }

    // private
    fn field_type_changed(&self) -> bool {
        for (field_name, field_type) in self.current_code_schema.fields.iter() {
            if let Some(db_field_type) = self.current_db_schema.fields.get(field_name) {
                if field_type.to_lowercase() != db_field_type.to_lowercase() {
                    return true;
                }
            }
        }

        false
    }

    fn panic_on_udt_fields_removal(&self) {
        if self.migration_object_type == MigrationObjectType::UDT {
            if self.removed_fields().len() > 0 {
                panic!("\n{}\n", "UDT fields removal is not allowed!".bold().bright_red());
            }
        }
    }

    fn panic_on_mv_fields_change(&self) {
        if self.migration_object_type == MigrationObjectType::MaterializedView {
            panic!("\n{}\n", "Materialized view fields change is not allowed!".bold().bright_red());
        }
    }

    fn partition_key_changed(&self) -> bool {
        self.current_code_schema.partition_keys.clone().sort() !=
            self.current_db_schema.partition_keys.clone().sort()
    }

    fn clustering_key_changed(&self) -> bool {
        self.current_code_schema.clustering_keys.clone().sort() !=
            self.current_db_schema.clustering_keys.clone().sort()
    }

}
