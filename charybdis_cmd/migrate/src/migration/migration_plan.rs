use colored::Colorize;
use scylla::Session;
use crate::migration::migration::{Migration, MigrationObjectType};
use crate::schema::{CurrentCodeSchema, CurrentDbSchema, SchemaObject};

pub(crate) struct MigrationPlan<'a> {
    current_db_schema: &'a CurrentDbSchema,
    current_code_schema: &'a CurrentCodeSchema,
    session: &'a Session,
}

impl <'a>MigrationPlan<'a> {
    pub(crate) fn new(current_db_schema: &'a CurrentDbSchema,
                      current_code_schema: &'a CurrentCodeSchema,
                      session: &'a Session) -> Self {
        MigrationPlan {
            current_db_schema,
            current_code_schema,
            session,
        }
    }

    pub(crate) async fn run(&self) {
        self.run_udts().await.unwrap();
        self.run_tables().await.unwrap();
        self.run_materialized_views().await.unwrap();


        println!("\n{}", "Migration plan ran successfully!".bright_green());
    }

    async fn run_udts(&self) -> Result<(), ()> {
        let empty_udt = SchemaObject::new();

        for (name, udt) in self.current_code_schema.udts.iter() {
            let migration = Migration::new(
                name,
                MigrationObjectType::UDT,
                udt,
                self.current_db_schema.udts.get(name).unwrap_or(&empty_udt),
                &self.session,
            );

            migration.run().await;
        }

        Ok(())
    }

    async fn run_tables(&self) -> Result<(), ()>  {
        let empty_udt = SchemaObject::new();

        for (name, table) in self.current_code_schema.tables.iter() {
            let migration = Migration::new(
                name,
                MigrationObjectType::Table,
                table,
                self.current_db_schema.tables.get(name).unwrap_or(&empty_udt),
                &self.session,
            );

            migration.run().await;
        }

        Ok(())
    }

    async fn run_materialized_views(&self) -> Result<(), ()>  {
        let empty_udt = SchemaObject::new();

        for (name, materialized_view) in self.current_code_schema.materialized_views.iter() {
            let migration = Migration::new(
                name,
                MigrationObjectType::MaterializedView,
                materialized_view,
                self.current_db_schema.materialized_views.get(name).unwrap_or(&empty_udt),
                &self.session,
            );

            migration.run().await;
        }

        Ok(())
    }
}
