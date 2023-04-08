use quote::quote;
use syn::ImplItem;
use crate::parser::CharybdisArgs;


// From Scylla docs:
// Prepared queries have good performance, much better than simple queries.
// By default they use shard/token aware load balancing.
// Always pass partition key values as bound values.
// Otherwise the driver can’t hash them to compute partition key and they will be sent
// to the wrong node, which worsens performance.
pub(crate) fn insert_query_const(ch_args: &CharybdisArgs) -> ImplItem {
    let mut primary_key = ch_args.partition_keys.clone().unwrap();
    let mut clustering_keys = ch_args.clustering_keys.clone().unwrap();
    let table_name = ch_args.table_name.as_ref().unwrap();

    primary_key.append(clustering_keys.as_mut());

    let primary_key_where_clause: String = primary_key.join(" = ? AND ");
    let query_str = format!(
        "INSERT INTO {} WHERE {}",
        table_name,
        primary_key_where_clause
    );


    let find_by_primary_key_query_const_str = quote! {
        const INSERT_QUERY: &'static str = #query_str;
    };

    syn::parse_quote!(#find_by_primary_key_query_const_str)
}
