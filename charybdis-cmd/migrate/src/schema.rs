pub(crate) mod code_schema_parser;
pub(crate) mod current_code_schema;
pub(crate) mod current_db_schema;

use charybdis_parser::LocalIndexTarget;
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub type IndexName = String;
pub type GlobalIndexTarget = String;

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SchemaObject {
    pub fields: HashMap<String, String>,
    pub type_name: String,
    pub table_name: String,
    pub base_table: String,
    pub partition_keys: Vec<String>,
    pub clustering_keys: Vec<String>,
    pub global_secondary_indexes: Vec<(IndexName, GlobalIndexTarget)>,
    pub local_secondary_indexes: Vec<(IndexName, LocalIndexTarget)>,
    pub table_options: Option<String>,
}

impl SchemaObject {
    pub fn new() -> Self {
        SchemaObject {
            fields: HashMap::new(),
            type_name: String::new(),
            table_name: String::new(),
            base_table: String::new(),
            partition_keys: Vec::new(),
            clustering_keys: Vec::new(),
            global_secondary_indexes: Vec::new(),
            local_secondary_indexes: Vec::new(),
            table_options: None,
        }
    }
}

pub type SchemaObjects = HashMap<String, SchemaObject>;

pub trait SchemaObjectTrait {
    fn get_cql_fields(&self) -> String;
}

impl SchemaObjectTrait for SchemaObject {
    fn get_cql_fields(&self) -> String {
        let mut cql_fields = String::new();
        let mut sorted_fields: Vec<(&String, &String)> = self.fields.iter().collect();
        sorted_fields.sort();

        for (field_name, field_type) in sorted_fields.iter() {
            cql_fields.push_str(&format!(
                "    {} {},\n",
                field_name.bright_cyan().bold(),
                field_type.bright_yellow()
            ));
        }

        cql_fields.pop();
        cql_fields.pop();
        cql_fields
    }
}
