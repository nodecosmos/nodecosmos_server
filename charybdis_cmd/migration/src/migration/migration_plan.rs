use colored::Colorize;
use scylla::Session;
use crate::migration::migration::{Migration, MigrationObjectType};
use crate::schema::{CurrentCodeSchema, CurrentDbSchema, SchemaObject};

pub struct MigrationPlan<'a> {
    current_db_schema: &'a CurrentDbSchema,
    current_code_schema: &'a CurrentCodeSchema,
    session: &'a Session,
}

impl <'a>MigrationPlan<'a> {
    pub fn new(current_db_schema: &'a CurrentDbSchema, current_code_schema: &'a CurrentCodeSchema, session: &'a Session)
        -> Self {
        MigrationPlan {
            current_db_schema,
            current_code_schema,
            session,
        }
    }

    pub fn run(&self) {
        self.run_udts();
        self.run_tables();
        self.run_materialized_views();

        println!("{}", "Migration plan ran successfully!".bright_green());
    }

    fn run_udts(&self) {
        let empty_udt = SchemaObject::new();

        self.current_code_schema.udts.iter().for_each(|(name, udt)| {
            let migration = Migration::new(
                name,
                MigrationObjectType::UDT,
                udt,
                self.current_db_schema.udts.get(name).unwrap_or(&empty_udt),
                &self.session,
            );

            migration.run();
        });
    }

    fn run_tables(&self) {
        let empty_udt = SchemaObject::new();

        self.current_code_schema.tables.iter().for_each(|(name, table)| {
            let migration = Migration::new(
                name,
                MigrationObjectType::Table,
                table,
                self.current_db_schema.tables.get(name).unwrap_or(&empty_udt),
                &self.session,
            );

            migration.run();
        });
    }

    fn run_materialized_views(&self) {
        let empty_udt = SchemaObject::new();

        self.current_code_schema.materialized_views.iter().for_each(|(name, materialized_view)| {
            let migration = Migration::new(
                name,
                MigrationObjectType::MaterializedView,
                materialized_view,
                self.current_db_schema.materialized_views.get(name).unwrap_or(&empty_udt),
                &self.session,
            );

            migration.run();
        });
    }
}
