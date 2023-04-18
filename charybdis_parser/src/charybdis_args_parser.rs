use proc_macro2::TokenStream;
use syn::{Attribute, DeriveInput};
use syn::parse::{Parse, ParseStream};
use crate::parse_fields_from_array::parse_array_expr;

#[derive(Debug)]
pub struct CharybdisArgs {
    pub table_name: Option<String>,
    pub type_name: Option<String>,
    pub base_table: Option<String>,
    pub partition_keys: Option<Vec<String>>,
    pub clustering_keys: Option<Vec<String>>,
    pub secondary_indexes: Option<Vec<String>>,
}

impl CharybdisArgs {
    pub fn from_derive(input: &DeriveInput) -> Self {
        let charybdis_model_attr: &Attribute = input.attrs
            .iter()
            .find(|attr| attr.path().is_ident("charybdis_model"))
            .unwrap_or_else(|| panic!("Missing charybdis_model attribute"));
        let args: CharybdisArgs = charybdis_model_attr
            .parse_args::<CharybdisArgs>()
            .unwrap();
        args
    }

    pub fn get_primary_key(&self) -> Vec<String> {
        let mut primary_key: Vec<String> = self.partition_keys.clone().unwrap();
        let mut clustering_keys: Vec<String> = self.clustering_keys.clone().unwrap();
        primary_key.append(clustering_keys.as_mut());
        primary_key
    }
}

impl Parse for CharybdisArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut type_name = None;
        let mut table_name = None;
        let mut base_table = None;
        let mut partition_keys = None;
        let mut clustering_keys = None;
        let mut secondary_indexes = None;

        while !input.is_empty() {
            let key: syn::Ident = input.parse()?;
            input.parse::<syn::Token![=]>()?;

            match key.to_string().as_str() {
                "type_name" => {
                    let value: syn::LitStr = input.parse()?;
                    type_name = Some(value.value());
                },
                "table_name" => {
                    let value: syn::LitStr = input.parse()?;
                    table_name = Some(value.value());
                }
                "base_table" => {
                    let value: syn::LitStr = input.parse()?;
                    base_table = Some(value.value());
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
            type_name,
            table_name,
            base_table,
            partition_keys,
            clustering_keys,
            secondary_indexes,
        })
    }
}

impl From<TokenStream> for CharybdisArgs {
    fn from(tokens: TokenStream) -> Self {
        // Convert the input tokens to a ParseStream
        let parse_stream: TokenStream = syn::parse2(tokens).unwrap();

        // Parse the ParseStream into a MyStruct instance
        let my_struct: CharybdisArgs = syn::parse2(parse_stream).unwrap();

        // Return the parsed MyStruct instance
        my_struct
    }
}
