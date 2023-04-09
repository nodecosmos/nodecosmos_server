use colored::Colorize;
use scylla::Session;

use crate::schema::SchemaObject;

#[derive(Debug, Eq, PartialEq)]
pub enum MigrationObjectType {
    UDT,
    Table,
    MaterializedView,
}

pub struct Migration<'a> {
    pub migration_object_name: &'a String,
    pub migration_object_type: MigrationObjectType,
    pub current_code_schema: &'a SchemaObject,
    pub current_db_schema: &'a SchemaObject,
    pub is_first_migration_run: bool,

    pub session: &'a Session,
}

impl <'a> Migration<'a>  {
    pub fn new(migration_object_name: &'a String,
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
    pub fn run (&self) {
        if self.is_first_migration_run {
            self.run_first_migration();
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
            self.run_field_added_migration();
            is_any_field_changed = true;
        }

        if self.removed_fields().len() > 0 {
            self.run_field_removed_migration();
            is_any_field_changed = true;
        }

        if self.migration_object_type != MigrationObjectType::UDT {
            if self.new_secondary_indexes().len() > 0 {
                is_any_field_changed = true;
                self.run_index_added_migration();
            }

            if self.removed_secondary_indexes().len() > 0 {
                is_any_field_changed = true;
                self.run_index_removed_migration();
            }
        }

        if !is_any_field_changed {
            println!(
                "{} {} {}",
                "No changes detected for ".bright_green(),
                self.migration_object_name.bright_yellow(),
                self.migration_obj_type_str().bright_yellow()
            );
        }

    }

    pub fn migration_obj_type_str(&self) -> String {
        match self.migration_object_type {
            MigrationObjectType::UDT => "UDT".to_string(),
            MigrationObjectType::Table => "Table".to_string(),
            MigrationObjectType::MaterializedView => "Materialized View".to_string(),
        }
    }

    pub fn new_fields(&self) -> Vec<String> {
        let mut new_fields: Vec<String> = vec![];

        for (field_name, _) in self.current_code_schema.fields.iter() {
            if !self.current_db_schema.fields.contains_key(field_name) {
                new_fields.push(field_name.clone());
            }
        }

        new_fields
    }

    pub fn removed_fields(&self) -> Vec<String> {
        let mut removed_fields: Vec<String> = vec![];

        for (field_name, _) in self.current_db_schema.fields.iter() {
            if !self.current_code_schema.fields.contains_key(field_name) {
                removed_fields.push(field_name.clone());
            }
        }

        removed_fields
    }


    pub fn new_secondary_indexes(&self) -> Vec<String> {
        let mut new_indexes: Vec<String> = vec![];

        self.current_code_schema.secondary_indexes.iter().for_each(|index| {
            if !self.current_db_schema.secondary_indexes.contains(index) {
                new_indexes.push(index.clone());
            }
        });

        new_indexes
    }

    pub fn removed_secondary_indexes(&self) -> Vec<String> {
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

    fn partition_key_changed(&self) -> bool {
        self.current_code_schema.partition_keys.clone().sort() != self.current_db_schema.partition_keys.clone().sort()
    }

    fn clustering_key_changed(&self) -> bool {
        self.current_code_schema.clustering_keys.clone().sort() != self.current_db_schema.clustering_keys.clone().sort()
    }

}
