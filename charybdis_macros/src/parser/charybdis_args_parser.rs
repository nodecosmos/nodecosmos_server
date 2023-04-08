use syn::parse::{Parse, ParseStream};
use crate::parser::parse_fields_from_array::parse_array_expr;

pub(crate) struct CharybdisArgs {
    pub table_name: Option<String>,
    pub partition_keys: Option<Vec<String>>,
    pub clustering_keys: Option<Vec<String>>,
    pub secondary_indexes: Option<Vec<String>>,
}

impl Parse for CharybdisArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut table_name = None;
        let mut partition_keys = None;
        let mut clustering_keys = None;
        let mut secondary_indexes = None;

        while !input.is_empty() {
            let key: syn::Ident = input.parse()?;
            input.parse::<syn::Token![=]>()?;

            match key.to_string().as_str() {
                "table_name" => {
                    let value: syn::LitStr = input.parse()?;
                    table_name = Some(value.value());
                }
                "partition_keys" => {
                    let array: syn::ExprArray = input.parse()?;
                    let parsed = parse_array_expr(array);
                    partition_keys = Some(parsed)
                }
                "clustering_keys" => {
                    let array: syn::ExprArray = input.parse()?;
                    let parsed = parse_array_expr(array);
                    clustering_keys = Some(parsed)
                }
                "secondary_indexes" => {
                    let array: syn::ExprArray = input.parse()?;
                    let parsed = parse_array_expr(array);
                    secondary_indexes = Some(parsed)
                }
                _ => {}
            }

            if !input.is_empty() {
                input.parse::<syn::Token![,]>()?;
            }
        }

        Ok(CharybdisArgs {
            table_name,
            partition_keys,
            clustering_keys,
            secondary_indexes,
        })
    }
}
