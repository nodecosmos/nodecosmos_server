use quote::quote;
use syn::ImplItem;
use crate::parser::CharybdisArgs;

pub(crate) fn find_by_primary_key_query_const(ch_args: &CharybdisArgs) -> ImplItem {
    let mut primary_key = ch_args.partition_keys.clone().unwrap();
    let mut clustering_keys = ch_args.clustering_keys.clone().unwrap();
    let table_name = ch_args.table_name.as_ref().unwrap();

    primary_key.append(clustering_keys.as_mut());

    let primary_key_where_clause: String = primary_key.join(" = ? AND ");
    let query_str = format!(
        "SELECT * FROM {} WHERE {} = ?",
        table_name.to_string(),
        primary_key_where_clause
    );


    let find_by_primary_key_query_const_str = quote! {
        const FIND_BY_PRIMARY_KEY_QUERY: &'static str = #query_str;
    };

    syn::parse_quote!(#find_by_primary_key_query_const_str)
}

pub(crate) fn find_by_partition_key_query_const(ch_args: &CharybdisArgs) -> ImplItem {
    let partition_keys = ch_args.partition_keys.clone().unwrap();
    let table_name = ch_args.table_name.as_ref().unwrap();

    let primary_key_where_clause: String = partition_keys.join(" = ? AND ");
    let query_str = format!(
        "SELECT * FROM {} WHERE {} = ?",
        table_name.to_string(),
        primary_key_where_clause
    );

    let find_by_partition_key_query_const_str = quote! {
        const FIND_BY_PARTITION_KEY_QUERY: &'static str = #query_str;
    };

    syn::parse_quote!(#find_by_partition_key_query_const_str)
}
