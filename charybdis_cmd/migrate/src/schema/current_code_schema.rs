use walkdir::{DirEntry, WalkDir};
use std::path::PathBuf;
use structopt::StructOpt;
use serde::{Serialize, Deserialize};

use super::schema::SchemaObject;
use super::schema::SchemaObjects;
use super::parser::*;

const MODEL_MACRO_NAME: &str = "charybdis_model";
const MATERIALIZED_VIEW_MACRO_NAME: &str = "charybdis_view_model";
const UDT_MACRO_NAME: &str = "charybdis_udt_model";

#[derive(Debug, StructOpt)]
#[structopt(name = "my_cli_tool", about = "A CLI tool for Rust projects.")]
struct Opt {
    /// The path to the project root directory.
    #[structopt(short, long, parse(from_os_str), default_value = ".")]
    project_root: PathBuf,
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct CurrentCodeSchema {
    pub tables: SchemaObjects,
    pub udts: SchemaObjects,
    pub materialized_views: SchemaObjects,
}

impl CurrentCodeSchema {
    pub fn new () -> CurrentCodeSchema {
        let mut current_code_schema = CurrentCodeSchema {
            tables: SchemaObjects::new(),
            udts: SchemaObjects::new(),
            materialized_views: SchemaObjects::new(),
        };

        current_code_schema.get_models_from_code();

        return current_code_schema;
    }

    pub fn get_models_from_code(&mut self) {
        let opt = Opt::from_args();

        // // In the get_models_from_code function:
        let project_root = opt.project_root;

        if let Some(models_base_dir) = find_src_models_dir(&project_root) {
            for entry in WalkDir::new(&models_base_dir) {
                let entry: DirEntry = entry.unwrap();
                if entry.path().is_file() {
                    if entry.path().to_str().unwrap().contains("mod.rs") {
                        continue;
                    }

                    if entry.path().to_str().unwrap().contains("materialized_views") {
                        self.populate_materialized_views(entry);
                    } else if entry.path().to_str().unwrap().contains("udts") {
                        self.populate_udts(entry);
                    } else {
                        self.populate_tables(entry);
                    }
                }
            }
        } else {
            eprintln!("Could not find 'src/models' directory.");
        }
    }

    pub fn populate_materialized_views(&mut self, entry: DirEntry) {
        let file_content: String = parse_file_as_string(entry.path());
        let schema_object: SchemaObject =
            parse_charybdis_model_def(&file_content, MATERIALIZED_VIEW_MACRO_NAME);
        let table_name = schema_object.table_name.clone();

        if table_name.is_empty() {
            panic!(
                "Could not find {} macro for file: {}",
                MATERIALIZED_VIEW_MACRO_NAME,
                entry.path().to_str().unwrap()
            );
        }

        self.materialized_views.insert(table_name, schema_object);
    }

    pub fn populate_udts(&mut self, entry: DirEntry) {
        let file_content: String = parse_file_as_string(entry.path());
        let schema_object: SchemaObject = parse_charybdis_model_def(&file_content, UDT_MACRO_NAME);
        let type_name = schema_object.type_name.clone();

        if type_name.is_empty() {
            panic!(
                "Could not find {} macro for file: {}",
                UDT_MACRO_NAME,
                entry.path().to_str().unwrap()
            );
        }

        self.udts.insert(type_name, schema_object);
    }

    pub fn populate_tables(&mut self, entry: DirEntry) {
        let file_content: String = parse_file_as_string(entry.path());
        let schema_object: SchemaObject = parse_charybdis_model_def(&file_content, MODEL_MACRO_NAME);
        let table_name = schema_object.table_name.clone();

        if table_name.is_empty() {
            panic!(
                "Could not find {} macro for file: {}",
                MODEL_MACRO_NAME,
                entry.path().to_str().unwrap()
            );
        }

        self.tables.insert(table_name, schema_object);
    }
}
