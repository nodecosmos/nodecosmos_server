use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use serde::{Serialize, Deserialize};
use walkdir::WalkDir;
use crate::schema::SchemaObject;
use super::schema::SchemaObjects;
use syn::{DeriveInput, Fields, Field, Item, ImplItem};
use std::path::PathBuf;
use structopt::StructOpt;

// ...

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
        let models_base_dir = "src/models";

        println!("models_base_dir: {}", models_base_dir);

        if let Some(models_base_dir) = find_src_models_dir(&project_root) {
            for entry in WalkDir::new(&models_base_dir) {
                let entry = entry.unwrap();
                if entry.path().is_file() {
                    if entry.path().to_str().unwrap().contains("parser") {
                        continue;
                    }

                    let file_content: String = read_file_to_string(entry.path());
                    let (schema_object, model_name):
                        (SchemaObject, Option<String>) = parse_struct_fields(file_content);

                    if model_name.is_none() {
                        panic!("Could not find model name for file: {}",
                               entry.path().to_str().unwrap());
                    }


                    if entry.path().to_str().unwrap().contains("materialized_views") {
                        self.materialized_views.insert(model_name.unwrap(), schema_object);
                    } else if entry.path().to_str().unwrap().contains("udts") {
                        self.udts.insert(model_name.unwrap(), schema_object);
                    } else {
                        self.tables.insert(model_name.unwrap(), schema_object);
                    }
                }
            }
        } else {
            eprintln!("Could not find 'src/models' directory.");
        }
    }
}
fn find_src_models_dir(project_root: &PathBuf) -> Option<PathBuf> {
    for entry in WalkDir::new(project_root) {
        let entry = entry.unwrap();
        if entry.file_type().is_dir() && entry.file_name().to_string_lossy() == "models" {
            let parent_dir = entry.path().parent()?;
            if parent_dir.file_name().unwrap().to_string_lossy() == "src" {
                return Some(entry.into_path());
            }
        }
    }
    None
}

fn read_file_to_string(path: &Path) -> String {
    let mut file_content = String::new();
    File::open(path)
        .unwrap()
        .read_to_string(&mut file_content)
        .unwrap();
    file_content
}


// fn parse_struct_fields(file_content: String) -> SchemaObject {
//     let ast = syn::parse_file(&file_content).unwrap();
//     let mut schema_object = SchemaObject::new();
//
//     for item in ast.items {
//         if let syn::Item::Struct(item_struct) = item {
//             if let Fields::Named(fields_named) = item_struct.fields {
//                 for field in fields_named.named {
//                     if let Field {
//                         ident: Some(ident),
//                         ty: syn::Type::Path(type_path),
//                         ..
//                     } = field
//                     {
//                         let field_name = ident.to_string();
//                         let field_type = type_path.path.segments[0].ident.to_string();
//                         schema_object.insert(field_name, field_type);
//                     }
//                 }
//             }
//         }
//     }
//
//     schema_object
// }

fn parse_struct_fields(file_content: String) -> (SchemaObject, Option<String>) {
    let ast = syn::parse_file(&file_content).unwrap();
    let mut schema_object: HashMap<String, String> = SchemaObject::new();
    let mut db_model_name: Option<String> = None;

    for item in ast.items {
        match item {
            Item::Struct(item_struct) => {
                if let Fields::Named(fields_named) = item_struct.fields {
                    for field in fields_named.named {
                        if let Field {
                            ident: Some(ident),
                            ty: syn::Type::Path(type_path),
                            ..
                        } = field
                        {
                            let field_name = ident.to_string();
                            let field_type = type_path.path.segments[0].ident.to_string();
                            schema_object.insert(field_name, field_type);
                        }
                    }
                }
            }
            Item::Impl(item_impl) => {
                for impl_item in item_impl.items {
                    if let syn::ImplItem::Const(const_item) = impl_item {
                        if const_item.ident == "DB_MODEL_NAME" {
                            if let syn::Expr::Lit(lit) = const_item.expr {
                                if let syn::Lit::Str(lit_str) = lit.lit {
                                    db_model_name = Some(lit_str.value());
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    (schema_object, db_model_name)
}


fn extract_const_value_from_impl_item(impl_item: ImplItem) -> Option<String> {
    if let syn::ImplItem::Const(const_item) = impl_item {
        if const_item.ident == "DB_MODEL_NAME" {
            if let syn::Expr::Lit(lit) = const_item.expr {
                if let syn::Lit::Str(lit_str) = lit.lit {
                    return Some(lit_str.value());
                }
            }
        }
    }
    None
}

