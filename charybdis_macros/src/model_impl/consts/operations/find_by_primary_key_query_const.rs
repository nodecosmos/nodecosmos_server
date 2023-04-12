use quote::quote;
use syn::{FieldsNamed, ImplItem};
use charybdis_parser::CharybdisArgs;

// TODO: query provided fields
pub(crate) fn find_by_primary_key_query_const(ch_args: &CharybdisArgs, fields_named: &FieldsNamed) -> ImplItem {
    let mut primary_key = ch_args.partition_keys.clone().unwrap();
    let mut clustering_keys = ch_args.clustering_keys.clone().unwrap();
    let table_name = ch_args.table_name.as_ref().unwrap();

    primary_key.append(clustering_keys.as_mut());

    let comma_sep_cols: String = fields_named.named
        .iter()
        .map(|field| field.ident.as_ref().unwrap().to_string())
        .collect::<Vec<String>>()
        .join(", ");

    let primary_key_where_clause: String = primary_key.join(" = ? AND ");
    let query_str = format!(
        "SELECT {} FROM {} WHERE {} = ?",
        comma_sep_cols,
        table_name.to_string(),
        primary_key_where_clause
    );


    let generated = quote! {
        const FIND_BY_PRIMARY_KEY_QUERY: &'static str = #query_str;
    };

    println!("{}", generated.to_string());

    syn::parse_quote!(#generated)
}

// TODO: query provided fields
pub(crate) fn find_by_partition_key_query_const(ch_args: &CharybdisArgs) -> ImplItem {
    let partition_keys = ch_args.partition_keys.clone().unwrap();
    let table_name = ch_args.table_name.as_ref().unwrap();

    let primary_key_where_clause: String = partition_keys.join(" = ? AND ");
    let query_str = format!(
        "SELECT * FROM {} WHERE {} = ?",
        table_name.to_string(),
        primary_key_where_clause
    );

    let generated = quote! {
        const FIND_BY_PARTITION_KEY_QUERY: &'static str = #query_str;
    };

    syn::parse_quote!(#generated)
}
