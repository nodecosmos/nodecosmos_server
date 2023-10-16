mod fields;
mod runner;

use crate::migration_unit::fields::MigrationUnitFields;
use crate::migration_unit::runner::MigrationUnitRunner;
use crate::schema::SchemaObject;
use colored::Colorize;
use scylla::Session;
use std::fmt::Display;

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub(crate) enum MigrationObjectType {
    Udt,
    Table,
    MaterializedView,
}

impl Display for MigrationObjectType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MigrationObjectType::Udt => write!(f, "UDT"),
            MigrationObjectType::Table => write!(f, "Table"),
            MigrationObjectType::MaterializedView => write!(f, "Materialized View"),
        }
    }
}

pub(crate) struct MigrationUnitData<'a> {
    pub(crate) migration_object_name: &'a String,
    pub(crate) migration_object_type: MigrationObjectType,
    pub(crate) current_code_schema: &'a SchemaObject,
    pub(crate) current_db_schema: &'a SchemaObject,
}

impl<'a> MigrationUnitData<'a> {
    pub(crate) fn is_first_migration(&self) -> bool {
        self.current_db_schema.fields.is_empty()
    }
}

pub(crate) struct MigrationUnit<'a> {
    pub(crate) data: &'a MigrationUnitData<'a>,
    pub(crate) fields: MigrationUnitFields<'a>,
    pub(crate) session: &'a Session,
}

impl<'a> MigrationUnit<'a> {
    pub(crate) fn new(data: &'a MigrationUnitData, session: &'a Session) -> Self {
        let fields = MigrationUnitFields::new(data);

        Self {
            data,
            fields,
            session,
        }
    }

    pub(crate) async fn run(&self) {
        let runner = MigrationUnitRunner::new(self);

        if self.data.is_first_migration() {
            runner.run_first_migration().await;
            runner.run_index_added_migration().await;

            return;
        }

        if self.fields.field_type_changed() {
            panic!("{}", "Field type change is not supported!".bright_red());

            // TODO: implement migration flag so on type change
            //  we drop existing column and create new one.
            //  Notify user that this will drop and recreate the column!
            // self.run_field_type_changed_migration();
        }

        self.panic_on_partition_key_change();
        self.panic_on_clustering_key_change();

        let mut is_any_field_changed = false;

        if !self.fields.new_fields.is_empty() {
            self.panic_on_mv_fields_change();

            runner.run_field_added_migration().await;
            is_any_field_changed = true;
        }

        if !self.fields.removed_fields.is_empty() {
            self.panic_on_mv_fields_change();
            self.panic_on_udt_fields_removal();

            runner.run_field_removed_migration().await;
            is_any_field_changed = true;
        }

        if self.data.migration_object_type != MigrationObjectType::Udt {
            if !self.fields.new_secondary_indexes.is_empty() {
                is_any_field_changed = true;
                runner.run_index_added_migration().await;
            }

            if !self.fields.removed_secondary_indexes.is_empty() {
                is_any_field_changed = true;
                runner.run_index_removed_migration().await;
            }
        }

        if !is_any_field_changed {
            println!(
                "{} {} {}",
                "No changes detected in".green(),
                self.data.migration_object_name.bright_yellow(),
                self.data.migration_object_type.to_string().bright_magenta()
            );
        }
    }

    fn panic_on_partition_key_change(&self) {
        if self.data.migration_object_type != MigrationObjectType::Udt {
            if self.fields.partition_key_changed() {
                panic!(
                    "\n\n{} {} {}\n{}\n\n",
                    "Illegal change in".bright_red(),
                    self.data.migration_object_name.bright_yellow(),
                    self.data.migration_object_type.to_string().bright_magenta(),
                    "Partition key change is not allowed!".bright_red(),
                );
            }
        }
    }

    fn panic_on_clustering_key_change(&self) {
        if self.data.migration_object_type != MigrationObjectType::Udt {
            if self.fields.clustering_key_changed() {
                panic!(
                    "\n\n{} {} {}\n{}\n\n",
                    "Illegal change in".bright_red(),
                    self.data.migration_object_name.bright_yellow(),
                    self.data.migration_object_type.to_string().bright_magenta(),
                    "Clustering key change is not allowed!".bright_red(),
                );
            }
        }
    }

    fn panic_on_udt_fields_removal(&self) {
        if self.data.migration_object_type == MigrationObjectType::Udt
            && !self.fields.removed_fields.is_empty()
        {
            panic!(
                "\n{}\n",
                "UDT fields removal is not allowed!".bold().bright_red()
            );
        }
    }

    fn panic_on_mv_fields_change(&self) {
        if self.data.migration_object_type == MigrationObjectType::MaterializedView {
            panic!(
                "\n{}\n",
                "Materialized view fields change is not allowed!"
                    .bold()
                    .bright_red()
            );
        }
    }
}
