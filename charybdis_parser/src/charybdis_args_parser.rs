use crate::parse_arr_expr_from_literals;
use crate::parse_fields_from_array::parse_array_expr;
use proc_macro2::TokenStream;
use std::collections::HashMap;
use syn::parse::{Parse, ParseStream};
use syn::{Attribute, DeriveInput};

#[derive(Debug)]
pub struct CharybdisArgs {
    pub table_name: Option<String>,
    pub type_name: Option<String>,
    pub base_table: Option<String>,
    pub partition_keys: Option<Vec<String>>,
    pub clustering_keys: Option<Vec<String>>,
    pub secondary_indexes: Option<Vec<String>>,
    pub fields_names: Option<Vec<String>>,
    pub field_types_hash: Option<HashMap<String, TokenStream>>,
    pub field_attributes_hash: Option<HashMap<String, TokenStream>>,
}

impl CharybdisArgs {
    pub fn from_derive(input: &DeriveInput) -> Self {
        let charybdis_model_attr: &Attribute = input
            .attrs
            .iter()
            .find(|attr| attr.path().is_ident("charybdis_model"))
            .unwrap_or_else(|| panic!("Missing charybdis_model attribute"));
        let args: CharybdisArgs = charybdis_model_attr.parse_args::<CharybdisArgs>().unwrap();
        args
    }

    pub fn get_primary_key(&self) -> Vec<String> {
        let mut primary_key: Vec<String> = self.partition_keys.clone().unwrap();
        let mut clustering_keys: Vec<String> = self.clustering_keys.clone().unwrap();

        primary_key.append(clustering_keys.as_mut());
        primary_key
    }

    pub fn hash_expr_lit_to_hash(
        expr: syn::Expr,
        cha_attr_name: String,
    ) -> HashMap<String, TokenStream> {
        // parse ruby style hash
        let hash = match expr {
            syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(lit_str),
                ..
            }) => lit_str.value(),
            _ => panic!("{} must be a string", cha_attr_name),
        };

        // hashmap
        let mut parsed_field_types_hash = HashMap::new();
        for pair in hash.split(";") {
            let pair = pair.trim();
            let pair: Vec<&str> = pair.split("=>").collect();

            if pair.len() != 2 {
                continue;
            }

            let key = pair[0].trim_matches('\'').trim();
            let value = pair[1].trim_matches('\'');

            // println!("key: {}", key);
            // println!("value: {}", value);

            let token = syn::parse_str::<TokenStream>(value).unwrap();

            parsed_field_types_hash.insert(key.to_string(), token);
        }

        parsed_field_types_hash
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
        let mut fields_names = None;
        let mut field_types_hash = None;
        let mut field_attributes_hash = None;

        while !input.is_empty() {
            let key: syn::Ident = input.parse()?;
            input.parse::<syn::Token![=]>()?;

            match key.to_string().as_str() {
                "type_name" => {
                    let value: syn::LitStr = input.parse()?;
                    type_name = Some(value.value());
                }
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
                "fields_names" => {
                    let array: syn::ExprArray = input.parse()?;
                    let parsed = parse_arr_expr_from_literals(array);

                    fields_names = Some(parsed)
                }
                "field_types_hash" => {
                    let hash: syn::Expr = input.parse()?;
                    let parsed_field_types_hash =
                        Self::hash_expr_lit_to_hash(hash, "field_types_hash".to_string());

                    field_types_hash = Some(parsed_field_types_hash);
                }
                "field_attributes_hash" => {
                    // parse ruby style hash
                    let hash: syn::Expr = input.parse()?;
                    let parsed_field_attributes_hash =
                        Self::hash_expr_lit_to_hash(hash, "field_attributes_hash".to_string());
                    field_attributes_hash = Some(parsed_field_attributes_hash);
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
            fields_names,
            field_types_hash,
            field_attributes_hash,
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
