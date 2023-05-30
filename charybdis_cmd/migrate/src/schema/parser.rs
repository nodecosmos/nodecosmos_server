use crate::schema::SchemaObject;
use charybdis_parser::CharybdisArgs;
use colored::Colorize;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use syn::{Field, Fields, GenericArgument, Item, PathArguments};
use walkdir::WalkDir;

// returns models dir if nested in src/models
pub(crate) fn find_src_models_dir(project_root: &PathBuf) -> Option<PathBuf> {
    for entry in WalkDir::new(project_root) {
        let entry = entry.unwrap();
        if entry.file_type().is_dir() && entry.file_name().to_string_lossy() == "models" {
            let parent_dir = entry.path().parent()?;
            if parent_dir.file_name().unwrap().to_string_lossy() == "src" {
                println!(
                    "{}\n",
                    "Detected 'src/models' directory".bright_green().bold()
                );
                return Some(entry.into_path());
            }
        }
    }
    None
}

pub(crate) fn parse_file_as_string(path: &Path) -> String {
    let mut file_content = String::new();
    File::open(path)
        .unwrap()
        .read_to_string(&mut file_content)
        .unwrap();
    file_content
}

pub(crate) fn parse_charybdis_model_def(file_content: &String, macro_name: &str) -> SchemaObject {
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
                            let field_type = type_with_arguments(&type_path);

                            schema_object.fields.insert(field_name, field_type);
                        }
                    }
                }

                // parse charybdis macro content
                for attr in &item_struct.attrs {
                    if attr.path().is_ident(macro_name) {
                        let args: CharybdisArgs = attr.parse_args().unwrap();

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

fn type_with_arguments(type_path: &syn::TypePath) -> String {
    let first_segment = &type_path.path.segments[0];

    // Check if the type is an Option<T>
    if first_segment.ident == "Option" {
        if let PathArguments::AngleBracketed(angle_bracketed_args) = &first_segment.arguments {
            if let Some(GenericArgument::Type(inner_type)) = angle_bracketed_args.args.first() {
                // Return the inner type of Option<T>
                return quote::quote! { #inner_type }.to_string();
            }
        }
    }

    // If not an Option<T>, return the type with arguments as a string
    return quote::quote! { #type_path }.to_string();
}
