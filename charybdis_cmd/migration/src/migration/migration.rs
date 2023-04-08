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

    pub partition_key: &'a String,
    pub clustering_key: &'a String,

    pub(crate) session: Session,
}

impl <'a> Migration<'a>  {
    pub fn new(migration_object_name: &'a String,
               migration_object_type: MigrationObjectType,
               current_code_schema: &'a SchemaObject,
               current_db_schema: &'a SchemaObject,
               partition_key: &'a String,
               clustering_key: &'a String,
               session: Session) -> Self {
        Self {
            migration_object_name,
            migration_object_type,
            current_code_schema,
            is_first_migration_run: current_db_schema.is_empty(),
            current_db_schema,
            partition_key,
            clustering_key,
            session,
        }
    }
    async fn run (&self) {
        if self.is_first_migration_run {
            self.run_first_migration().await;
            return;
        }

        if self.migration_object_type == MigrationObjectType::MaterializedView {
            return;
        }

        if self.field_type_changed() {
            panic!("Field type changed is not supported yet!");
            // TODO: implement question to user if he wants to continue
            //  notify user that this will drop the column and create it again

            // self.run_field_type_changed_migration().await;
        }

        let mut is_any_field_changed = false;

        if self.new_fields().len() > 0 {
            self.run_field_added_migration().await;
            is_any_field_changed = true;
        }

        if self.removed_fields().len() > 0 {
            self.run_field_removed_migration().await;
            is_any_field_changed = true;
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

        for (field_name, _) in self.current_code_schema.iter() {
            if !self.current_db_schema.contains_key(field_name) {
                new_fields.push(field_name.clone());
            }
        }

        new_fields
    }

    pub fn removed_fields(&self) -> Vec<String> {
        let mut removed_fields: Vec<String> = vec![];

        for (field_name, _) in self.current_db_schema.iter() {
            if !self.current_code_schema.contains_key(field_name) {
                removed_fields.push(field_name.clone());
            }
        }

        removed_fields
    }


    // private

    fn field_type_changed(&self) -> bool {
        for (field_name, field_type) in self.current_code_schema.iter() {
            if let Some(db_field_type) = self.current_db_schema.get(field_name) {
                if field_type != db_field_type {
                    return true;
                }
            }
        }

        false
    }
}
