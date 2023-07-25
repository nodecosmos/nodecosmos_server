use charybdis_parser::CharybdisArgs;
use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::{parse_str, FieldsNamed};

pub(crate) fn push_to_set_fields_query_consts(
    ch_args: &CharybdisArgs,
    fields_named: &FieldsNamed,
) -> TokenStream {
    let table_name = ch_args.table_name.as_ref().unwrap();

    let mut primary_key = ch_args.partition_keys.clone().unwrap();
    let mut clustering_keys = ch_args.clustering_keys.clone().unwrap();

    primary_key.append(clustering_keys.as_mut());

    let primary_key_where_clause: String = primary_key.join(" = ? AND ");

    // for each field in the struct, generate a macro that adds query to append value to cql set
    let queries: Vec<TokenStream> = fields_named
        .named
        .iter()
        .filter_map(|field| {
            let field_name = field.ident.as_ref().unwrap().to_string();
            let field_type = field.ty.to_token_stream().to_string();

            let is_list = field_type.contains("List");
            let is_set = field_type.contains("Set");

            if !is_list && !is_set {
                return None;
            }

            let query_str = if is_list {
                format!(
                    "UPDATE {} SET {} = {} + [?] WHERE {} = ?",
                    table_name.to_string(),
                    field_name,
                    field_name,
                    primary_key_where_clause,
                )
            } else {
                format!(
                    "UPDATE {} SET {} = {} + {{?}} WHERE {} = ?",
                    table_name.to_string(),
                    field_name,
                    field_name,
                    primary_key_where_clause,
                )
            };

            let field_name_upper = field_name.to_uppercase();
            let const_name = format!("PUSH_TO_{}_QUERY", field_name_upper);
            let const_name: TokenStream = parse_str::<TokenStream>(&const_name).unwrap();

            let expanded = quote! {
                pub const #const_name: &'static str = #query_str;
            };

            Some(expanded)
        })
        .collect();

    let expanded = quote! {
        #(#queries)*
    };

    expanded
}

pub(crate) fn pull_from_set_fields_query_consts(
    ch_args: &CharybdisArgs,
    fields_named: &FieldsNamed,
) -> TokenStream {
    let table_name = ch_args.table_name.as_ref().unwrap();

    let mut primary_key = ch_args.partition_keys.clone().unwrap();
    let mut clustering_keys = ch_args.clustering_keys.clone().unwrap();

    primary_key.append(clustering_keys.as_mut());

    let primary_key_where_clause: String = primary_key.join(" = ? AND ");

    // for each field in the struct, generate a macro that adds query to append value to cql set
    let queries: Vec<TokenStream> = fields_named
        .named
        .iter()
        .filter_map(|field| {
            let field_name = field.ident.as_ref().unwrap().to_string();
            let field_type = field.ty.to_token_stream().to_string();

            let is_list = field_type.contains("List");
            let is_set = field_type.contains("Set");

            if !is_list && !is_set {
                return None;
            }

            let query_str = if is_list {
                format!(
                    "UPDATE {} SET {} = {} - [?] WHERE {} = ?",
                    table_name.to_string(),
                    field_name,
                    field_name,
                    primary_key_where_clause,
                )
            } else {
                format!(
                    "UPDATE {} SET {} = {} - {{?}} WHERE {} = ?",
                    table_name.to_string(),
                    field_name,
                    field_name,
                    primary_key_where_clause,
                )
            };

            let field_name_upper = field_name.to_uppercase();
            let const_name = format!("PULL_FROM_{}_QUERY", field_name_upper);
            let const_name: TokenStream = parse_str::<TokenStream>(&const_name).unwrap();

            let expanded = quote! {
                pub const #const_name: &'static str = #query_str;
            };

            Some(expanded)
        })
        .collect();

    let expanded = quote! {
        #(#queries)*
    };

    expanded
}
