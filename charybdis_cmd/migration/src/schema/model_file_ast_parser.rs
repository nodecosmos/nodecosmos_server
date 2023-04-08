use syn::Attribute;
use crate::schema::SchemaObject;

#[derive(Default)]
pub struct ModelFileAstParser {
    ast: Option<syn::File>,
    db_model_name: Option<String>,
    partition_keys: Option<Vec<String>>,
    clustering_keys: Option<Vec<String>>,
}

impl ModelFileAstParser {
    pub fn new(file_content: String) -> Self {
        let ast = syn::parse_file(&file_content).unwrap();
        ModelFileAstParser {
            ast: Some(ast),
            ..Default::default()
        }
    }

    fn has_my_macro(attrs: &[Attribute]) -> bool {
        attrs.iter().any(|attr| attr.path().is_ident("charybdis"))
    }
}
