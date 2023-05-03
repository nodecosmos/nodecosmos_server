use charybdis_parser::CharybdisArgs;
use quote::quote;
use syn::{FieldsNamed, ImplItem};

pub(crate) fn update_query_const(ch_args: &CharybdisArgs, fields_named: &FieldsNamed) -> ImplItem {
    let table_name = ch_args.table_name.as_ref().unwrap();
    let primary_key = ch_args.get_primary_key();

    let primary_key_where_clause: String = primary_key.join(" = ? AND ");

    let mut set_fields_clause: String = fields_named
        .named
        .iter()
        .map(|field| field.ident.as_ref().unwrap().to_string())
        .filter(|field| !primary_key.contains(field))
        .collect::<Vec<String>>()
        .join(" = ?, ");

    set_fields_clause.push_str(" = ?");

    let query_str: String = format!(
        "UPDATE {} SET {} WHERE {} = ?",
        table_name, set_fields_clause, primary_key_where_clause,
    );

    let generated = quote! {
        const UPDATE_QUERY: &'static str = #query_str;
    };

    syn::parse_quote!(#generated)
}
