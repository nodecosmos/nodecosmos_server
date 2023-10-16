extern crate proc_macro;
mod helpers;
mod macro_rules;
mod model_impl;
mod native;
use crate::macro_rules::*;
use crate::model_impl::*;
use crate::native::{
    delete_by_clustering_key_functions, find_by_primary_keys_functions,
    pull_from_collection_fields_query_consts, push_to_collection_fields_query_consts,
};
use charybdis_parser::{parse_named_fields, CharybdisArgs};
use proc_macro::TokenStream;
use quote::quote;
use syn::parse_macro_input;
use syn::DeriveInput;

/// This macro generates the implementation of the [Model] trait for the given struct.
#[proc_macro_attribute]
pub fn charybdis_model(args: TokenStream, input: TokenStream) -> TokenStream {
    let args: CharybdisArgs = parse_macro_input!(args);
    let input: DeriveInput = parse_macro_input!(input);

    let struct_name = &input.ident;
    let fields_named = parse_named_fields(&input);

    // basic consts
    let db_model_name_const = db_model_name_const(&args);
    let partition_keys_const = partition_keys_const(&args);
    let clustering_keys_const = clustering_keys_const(&args);
    let primary_key_const = primary_key_const(&args);
    let secondary_indexes_const = secondary_indexes_const(&args);
    let select_fields_clause = select_fields_clause(&args, fields_named);

    // operation consts
    let find_by_primary_key_query_const = find_by_primary_key_query_const(&args, fields_named);
    let find_by_partition_key_query_const = find_by_partition_key_query_const(&args, fields_named);
    let insert_query_const = insert_query_const(&args, fields_named);
    let update_query_const = update_query_const(&args, fields_named);
    let delete_query_const = delete_query_const(&args);
    let delete_by_partition_key_query_const = delete_by_partition_key_query_const(&args);

    // model specific operation consts
    let push_to_collection_fields_query_consts =
        push_to_collection_fields_query_consts(&args, fields_named);
    let pull_from_collection_fields_query_consts =
        pull_from_collection_fields_query_consts(&args, fields_named);

    // methods
    let get_primary_key_values = get_primary_key_values(&args);
    let get_partition_key_values = get_partition_key_values(&args);
    let get_clustering_key_values = get_clustering_key_values(&args);
    let get_update_values = get_update_values(&args, fields_named);

    // rules
    let find_model_query_rule = find_model_query_rule(&args, fields_named, struct_name);
    let find_model_rule = find_model_rule(&args, fields_named, struct_name);
    let find_one_model_rule = find_one_model_rule(&args, fields_named, struct_name);
    let update_model_query_rule = update_model_query_rule(&args, struct_name);

    // Associated functions for finding by partial primary key
    let find_by_key_funs = find_by_primary_keys_functions(&args, fields_named, struct_name);
    let delete_by_cks_funs = delete_by_clustering_key_functions(&args, fields_named, struct_name);

    let partial_model_generator = partial_model_macro_generator(args, &input);

    let expanded = quote! {
        #[derive(charybdis::ValueList, charybdis::FromRow)]
        #input

        impl #struct_name {
            #find_by_key_funs
            #delete_by_cks_funs
            #push_to_collection_fields_query_consts
            #pull_from_collection_fields_query_consts
        }

       impl charybdis::model::BaseModel for #struct_name {
            // consts
            #db_model_name_const
            #clustering_keys_const
            #partition_keys_const
            #primary_key_const
            #find_by_primary_key_query_const
            #find_by_partition_key_query_const
            #select_fields_clause
            // methods
            #get_primary_key_values
            #get_partition_key_values
            #get_clustering_key_values
        }

        impl charybdis::model::Model for #struct_name {
            // consts
            #secondary_indexes_const
            // operation consts
            #insert_query_const
            #update_query_const
            #delete_query_const
            #delete_by_partition_key_query_const
            // methods
            #get_update_values
        }

        #find_model_query_rule
        #find_model_rule
        #find_one_model_rule
        #update_model_query_rule
        #partial_model_generator
    };

    TokenStream::from(expanded)
}

/// Generates the implementation of the MaterializedView trait
/// for the given struct.
#[proc_macro_attribute]
pub fn charybdis_view_model(args: TokenStream, input: TokenStream) -> TokenStream {
    let args: CharybdisArgs = parse_macro_input!(args);
    let input: DeriveInput = parse_macro_input!(input);

    let struct_name = &input.ident;
    let fields_named = parse_named_fields(&input);

    // Model consts
    let db_model_name_const = db_model_name_const(&args);
    let partition_keys_const = partition_keys_const(&args);
    let clustering_keys_const = clustering_keys_const(&args);
    let primary_key_const = primary_key_const(&args);
    let find_by_primary_key_query_const = find_by_primary_key_query_const(&args, fields_named);
    let find_by_partition_key_query_const = find_by_partition_key_query_const(&args, fields_named);
    let select_fields_clause = select_fields_clause(&args, fields_named);

    // Model methods
    let get_primary_key_values = get_primary_key_values(&args);
    let get_partition_key_values = get_partition_key_values(&args);
    let get_clustering_key_values = get_clustering_key_values(&args);

    // rules
    let find_model_query_rule = find_model_query_rule(&args, fields_named, struct_name);

    // Associated functions for finding by clustering keys
    let find_by_key_funs = find_by_primary_keys_functions(&args, fields_named, struct_name);

    let expanded = quote! {
        #[derive(charybdis::ValueList, charybdis::FromRow)]
        #input

        impl #struct_name {
            #find_by_key_funs
        }

        impl charybdis::model::BaseModel for #struct_name {
            // consts
            #db_model_name_const
            #clustering_keys_const
            #partition_keys_const
            #primary_key_const
            #find_by_primary_key_query_const
            #find_by_partition_key_query_const
            #select_fields_clause
            // methods
            #get_primary_key_values
            #get_partition_key_values
            #get_clustering_key_values
        }

        impl charybdis::model::MaterializedView for #struct_name {}

        #find_model_query_rule
    };

    TokenStream::from(expanded)
}

#[proc_macro_attribute]
pub fn charybdis_udt_model(_: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    // parse fields sorted by name
    let fields_named = parse_named_fields(&input);
    let struct_name = &input.ident;
    let named = &fields_named.named;

    // sort fields by name
    // https://github.com/scylladb/scylla-rust-driver/issues/370
    let mut sorted_fields: Vec<_> = named.into_iter().collect();
    sorted_fields.sort_by(|a, b| a.ident.as_ref().unwrap().cmp(b.ident.as_ref().unwrap()));

    let gen = quote! {
        #[derive(charybdis::FromUserType, charybdis::IntoUserType, Clone)]
        pub struct #struct_name {
            #(#sorted_fields),*
        }
    };

    gen.into()
}

#[proc_macro_attribute]
pub fn char_model_field_attrs_gen(args: TokenStream, input: TokenStream) -> TokenStream {
    let args: CharybdisArgs = parse_macro_input!(args);
    let input: DeriveInput = parse_macro_input!(input);

    let tkn_2 = char_model_field_attrs_macro_gen(args, input);

    tkn_2.into()
}
