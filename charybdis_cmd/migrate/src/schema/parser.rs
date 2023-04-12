use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use colored::Colorize;
use syn::{Field, Fields, Item};
use walkdir::WalkDir;
use charybdis_parser::CharybdisArgs;
use crate::schema::SchemaObject;

// returns models dir if nested in src/models
pub(crate) fn find_src_models_dir(project_root: &PathBuf) -> Option<PathBuf> {
    for entry in WalkDir::new(project_root) {
        let entry = entry.unwrap();
        if entry.file_type().is_dir() && entry.file_name().to_string_lossy() == "models" {
            let parent_dir = entry.path().parent()?;
            if parent_dir.file_name().unwrap().to_string_lossy() == "src" {
                println!("{}\n", "Detected 'src/models' directory".bright_green().bold());
                return Some(entry.into_path());
            }
        }
    }
    None
}

pub(crate)fn parse_file_as_string(path: &Path) -> String {
    let mut file_content = String::new();
    File::open(path)
        .unwrap()
        .read_to_string(&mut file_content)
        .unwrap();
    file_content
}

pub(crate)fn parse_charybdis_model_def(file_content: &String, macro_name: &str)
    -> SchemaObject {
    let ast: syn::File = syn::parse_file(&file_content).unwrap();
    let mut schema_object: SchemaObject = SchemaObject::new();

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
                            schema_object.fields.insert(field_name, field_type);
                        }
                    }
                }

                // parse charybdis macro content
                for attr in &item_struct.attrs {
                    if attr.path().is_ident(macro_name) {
                        let args = attr.parse_args::<CharybdisArgs>().unwrap();

                        schema_object.table_name = args.table_name.unwrap_or("".to_string());
                        schema_object.type_name = args.type_name.unwrap_or("".to_string());
                        schema_object.base_table = args.base_table.unwrap_or("".to_string());

                        schema_object.partition_keys = args.partition_keys.unwrap_or(vec![]);
                        schema_object.clustering_keys = args.clustering_keys.unwrap_or(vec![]);
                        schema_object.secondary_indexes = args.secondary_indexes.unwrap_or(vec![]);
                    }
                }
            }
            _ => {}
        }
    }

    schema_object
}
