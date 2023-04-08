use std::collections::HashMap;
use crate::migration::migration::{Migration, MigrationObjectType};
use crate::schema::{CurrentCodeSchema, CurrentDbSchema, SchemaObject};

pub struct MigrationPlan<'a> {
    current_db_schema: &'a CurrentDbSchema,
    current_code_schema: &'a CurrentCodeSchema,
    udts: HashMap<String, bool>,
}

impl <'a>MigrationPlan<'a> {
    pub fn new(current_db_schema: &'a CurrentDbSchema, current_code_schema: &'a CurrentCodeSchema) -> Self {
        MigrationPlan {
            current_db_schema: current_db_schema.clone(),
            current_code_schema: current_code_schema.clone(),
            udts: HashMap::new(),
        }
    }

    pub fn run(&self) {
        // First run udt migrations
        // Then run table migrations
        // Then run materialized view migrations



    }

    fn run_udts(&self) {
        self.current_code_schema.udts.iter().for_each(|(name, udt)| {
            let migration = Migration::new(
                name,
                MigrationObjectType::Udt,
                udt,
                self.current_db_schema.udts.get(name),
            );
            // let migration = Migration::new(
            //
            // );
        });
    }
}
