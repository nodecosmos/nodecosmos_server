use quote::quote;
use syn::{FieldsNamed, ImplItem};
use crate::parser::CharybdisArgs;


// From Scylla docs:
// Prepared queries have good performance, much better than simple queries.
// By default they use shard/token aware load balancing.
// Always pass partition key values as bound values.
// Otherwise the driver can’t hash them to compute partition key and they will be sent
// to the wrong node, which worsens performance.
pub(crate) fn insert_query_const(ch_args: &CharybdisArgs, fields_named: &FieldsNamed) -> ImplItem {
    let table_name = ch_args.table_name.as_ref().unwrap();

    let comma_sep_cols: String = fields_named
        .named
        .iter()
        .map(|field| field.ident.as_ref().unwrap().to_string())
        .collect::<Vec<String>>()
        .join(", ");

    let coma_sep_values_placeholders: String = fields_named
        .named
        .iter()
        .map(|_| "?".to_string())
        .collect::<Vec<String>>()
        .join(", ");

    let query_str: String = format!(
        "INSERT INTO {} ({}) VALUES ({})",
        table_name,
        comma_sep_cols,
        coma_sep_values_placeholders,
    );


    let find_by_primary_key_query_const_str = quote! {
        const INSERT_QUERY: &'static str = #query_str;
    };

    syn::parse_quote!(#find_by_primary_key_query_const_str)
}
