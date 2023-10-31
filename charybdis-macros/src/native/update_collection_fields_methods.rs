use crate::utils::serialized_field_value_adder;
use charybdis_parser::macro_args::CharybdisMacroArgs;
use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::{parse_str, Field};

// Here we utilize PUSH_TO_{}_QUERY and PULL_FROM_{}_QUERY consts to generate Model functions
// for updating collection fields.
pub fn push_to_collection_funs(ch_args: &CharybdisMacroArgs, fields: &Vec<Field>) -> TokenStream {
    let primary_key = ch_args.get_primary_key();
    let values_capacity: usize = primary_key.len() + 1; // +1 for the value to be pushed
    let serialized_field_value_adder: TokenStream = serialized_field_value_adder(primary_key);

    let push_to_collection_rules: Vec<TokenStream> = fields
        .iter()
        .filter_map(|field| {
            let field_name = field.ident.as_ref().unwrap().to_string();
            let field_type = field.ty.to_token_stream().to_string();

            let is_list = field_type.contains("List");
            let is_set = field_type.contains("Set");

            if !is_list && !is_set {
                return None;
            }

            let field_name_upper = field_name.to_uppercase();

            let push_to_query_str = format!("Self::PUSH_TO_{}_QUERY", field_name_upper);
            let push_to_query = parse_str::<TokenStream>(&push_to_query_str).unwrap();

            let fun_name_str = format!("push_to_{}", field_name);
            let fun_name = parse_str::<TokenStream>(&fun_name_str).unwrap();

            let expanded = quote! {
                pub async fn #fun_name(
                    &self,
                    session: &charybdis::CachingSession,
                    value: &impl charybdis::Value
                ) -> Result<(), charybdis::errors::CharybdisError> {
                    let mut serialized = charybdis::SerializedValues::with_capacity(#values_capacity);

                    serialized.add_value(value)?;

                    #serialized_field_value_adder


                    charybdis::operations::execute(session, #push_to_query, serialized).await?;

                    Ok(())
                }
            };

            Some(expanded)
        })
        .collect();

    let expanded = quote! {
        #(#push_to_collection_rules)*
    };

    expanded
}

pub fn pull_from_collection_funs(ch_args: &CharybdisMacroArgs, fields: &Vec<Field>) -> TokenStream {
    let primary_key = ch_args.get_primary_key();
    let values_capacity: usize = primary_key.len() + 1; // +1 for the value to be pushed
    let serialized_field_value_adder: TokenStream = serialized_field_value_adder(primary_key);

    let pull_from_collection_rules: Vec<TokenStream> = fields
        .iter()
        .filter_map(|field| {
            let field_name = field.ident.as_ref().unwrap().to_string();
            let field_type = field.ty.to_token_stream().to_string();

            let is_list = field_type.contains("List");
            let is_set = field_type.contains("Set");

            if !is_list && !is_set {
                return None;
            }

            let field_name_upper = field_name.to_uppercase();

            let pull_from_query_str = format!("Self::PULL_FROM_{}_QUERY", field_name_upper);
            let pull_from_query = parse_str::<TokenStream>(&pull_from_query_str).unwrap();

            let fun_name_str = format!("pull_from_{}", field_name);
            let fun_name = parse_str::<TokenStream>(&fun_name_str).unwrap();

            let expanded = quote! {
                pub async fn #fun_name(
                    &self,
                    session: &charybdis::CachingSession,
                    value: &impl charybdis::Value
                ) -> Result<(), charybdis::errors::CharybdisError> {
                    let mut serialized = charybdis::SerializedValues::with_capacity(#values_capacity);

                    serialized.add_value(value)?;

                    #serialized_field_value_adder


                    charybdis::operations::execute(session, #pull_from_query, serialized).await?;

                    Ok(())
                }
            };

            Some(expanded)
        })
        .collect();

    let expanded = quote! {
        #(#pull_from_collection_rules)*
    };

    expanded
}
