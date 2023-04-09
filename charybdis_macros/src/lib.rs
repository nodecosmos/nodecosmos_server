extern crate proc_macro;

mod parser;
mod impl_model;
mod macro_rules;
mod helpers;

use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, FieldsNamed, ImplItem};
use syn::parse_macro_input;

use crate::impl_model::*;
use crate::macro_rules::partial_model_rule::partial_model_macro_generator;
use crate::parser::{CharybdisArgs, parse_named_fields};

/// This macro generates the following constants:
/// - `DB_MODEL_NAME`
/// - `PARTITION_KEYS`
/// - `CLUSTERING_KEYS`
/// - `PRIMARY_KEY`
/// - `SECONDARY_INDEXES`
///
/// - `FIND_BY_PRIMARY_KEY_QUERY`
/// - `FIND_BY_PARTITION_KEY_QUERY`
/// - `INSERT_QUERY`
/// - `UPDATE_QUERY`
/// - `DELETE_QUERY`
///
///
/// This macro generates the following methods:
/// - `get_primary_key_values`
/// - `get_partition_key_values`
/// - `get_clustering_key_values`
#[proc_macro_attribute]
pub fn charybdis_model(args: TokenStream, input: TokenStream) -> TokenStream {
    let args: CharybdisArgs = parse_macro_input!(args);
    let input: DeriveInput = parse_macro_input!(input);

    let struct_name: &proc_macro2::Ident = &input.ident;
    let fields_named: &FieldsNamed = parse_named_fields(&input);

    // consts generators
    let db_model_name_const: ImplItem = db_model_name_const(&args);
    let partition_keys_const: ImplItem = partition_keys_const(&args);
    let clustering_keys_const: ImplItem = clustering_keys_const(&args);
    let primary_key_const: ImplItem = primary_key_const(&args);
    let secondary_indexes_const: ImplItem = secondary_indexes_const(&args);
    let find_by_primary_key_query_const: ImplItem = find_by_primary_key_query_const(&args);
    let find_by_partition_key_query_const: ImplItem = find_by_partition_key_query_const(&args);
    let insert_query_const: ImplItem = insert_query_const(&args, fields_named);

    // methods generators
    let get_primary_key_values: ImplItem = get_primary_key_values(&args);
    let get_partition_key_values: ImplItem = get_partition_key_values(&args);
    let get_clustering_key_values: ImplItem = get_clustering_key_values(&args);

    let expanded = quote! {
        #[derive(
            charybdis::prelude::Serialize,
            charybdis::prelude::Deserialize,
            charybdis::prelude::FromRow,
            charybdis::prelude::ValueList,
            Default,
            Debug
        )]
        #input

        impl charybdis::prelude::Model for #struct_name {
            // consts
            #db_model_name_const
            #clustering_keys_const
            #partition_keys_const
            #primary_key_const
            #secondary_indexes_const
            #find_by_primary_key_query_const
            #find_by_partition_key_query_const
            #insert_query_const
            // methods
            #get_primary_key_values
            #get_partition_key_values
            #get_clustering_key_values
        }
    };

    TokenStream::from(expanded)
}

#[proc_macro_attribute]
pub fn partial_model_generator(_: TokenStream, input: TokenStream) -> TokenStream {
    partial_model_macro_generator(input)
}

/// This macro generates the following constants:
/// - `DB_MODEL_NAME`
/// - `PARTITION_KEYS`
/// - `CLUSTERING_KEYS`
/// - `PRIMARY_KEY`
/// - `FIND_BY_PRIMARY_KEY_QUERY`
/// - `FIND_BY_PARTITION_KEY_QUERY`
///
/// This macro generates the following methods:
/// - `get_primary_key_values`
/// - `get_partition_key_values`
/// - `get_clustering_key_values`
#[proc_macro_attribute]
pub fn charybdis_view_model(args: TokenStream, input: TokenStream) -> TokenStream {
    let args: CharybdisArgs = parse_macro_input!(args);
    let input: DeriveInput = parse_macro_input!(input);

    let struct_name = &input.ident;

    // consts
    let db_model_name_const: ImplItem = db_model_name_const(&args);
    let partition_keys_const: ImplItem = partition_keys_const(&args);
    let clustering_keys_const: ImplItem = clustering_keys_const(&args);
    let primary_key_const: ImplItem = primary_key_const(&args);
    let find_by_primary_key_query_const: ImplItem = find_by_primary_key_query_const(&args);
    let find_by_partition_key_query_const: ImplItem = find_by_partition_key_query_const(&args);

    // methods
    let get_primary_key_values = get_primary_key_values(&args);
    let get_partition_key_values = get_partition_key_values(&args);
    let get_clustering_key_values = get_clustering_key_values(&args);

    let expanded = quote! {
        #[derive(
            charybdis::prelude::Serialize,
            charybdis::prelude::Deserialize,
            charybdis::prelude::FromRow,
            charybdis::prelude::ValueList,
            Default,
            Debug
        )]
        #input

        impl charybdis::prelude::MaterializedView for #struct_name {
            // consts
            #db_model_name_const
            #clustering_keys_const
            #partition_keys_const
            #primary_key_const
            #find_by_primary_key_query_const
            #find_by_partition_key_query_const
            // methods
            #get_primary_key_values
            #get_partition_key_values
            #get_clustering_key_values
        }
    };

    TokenStream::from(expanded)
}

#[proc_macro_attribute]
pub fn charybdis_udt_model(_: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let gen = quote! {
        #[derive(
            charybdis::prelude::Serialize,
            charybdis::prelude::Deserialize,
            charybdis::prelude::FromUserType,
            charybdis::prelude::IntoUserType,
            PartialEq,
            Default,
            Debug,
        )]
        #input
    };

    gen.into()
}
