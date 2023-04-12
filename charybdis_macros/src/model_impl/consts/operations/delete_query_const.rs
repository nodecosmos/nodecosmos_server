use quote::quote;
use syn::{FieldsNamed, ImplItem};
use charybdis_parser::CharybdisArgs;

pub(crate) fn delete_query_const(ch_args: &CharybdisArgs, fields_named: &FieldsNamed) -> ImplItem {
    let mut primary_key: Vec<String> = ch_args.partition_keys.clone().unwrap();
    let mut clustering_keys: Vec<String> = ch_args.clustering_keys.clone().unwrap();

    primary_key.append(clustering_keys.as_mut());

    let table_name: &String = ch_args.table_name.as_ref().unwrap();

    let comma_sep_cols: String = fields_named.named
        .iter()
        .map(|field| field.ident.as_ref().unwrap().to_string())
        .filter(|field| !primary_key.contains(field))
        .collect::<Vec<String>>()
        .join(", ");

    let primary_key_where_clause: String = primary_key.join(" = ? AND ");

    let query_str: String = format!(
        "DELETE {} FROM {} WHERE {} = ?",
        comma_sep_cols,
        table_name,
        primary_key_where_clause,
    );

    let generated = quote! {
        const DELETE_QUERY: &'static str = #query_str;
    };

    syn::parse_quote!(#generated)
}
