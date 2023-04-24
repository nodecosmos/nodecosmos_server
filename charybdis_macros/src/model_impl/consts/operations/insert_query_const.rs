use charybdis_parser::CharybdisArgs;
use quote::quote;
use syn::{FieldsNamed, ImplItem};

use crate::helpers::comma_sep_cols;

pub(crate) fn insert_query_const(ch_args: &CharybdisArgs, fields_named: &FieldsNamed) -> ImplItem {
    let table_name = ch_args.table_name.as_ref().unwrap();
    let comma_sep_cols = comma_sep_cols(fields_named);
    let coma_sep_values_placeholders: String = fields_named
        .named
        .iter()
        .map(|_| "?".to_string())
        .collect::<Vec<String>>()
        .join(", ");
    let query_str: String = format!(
        "INSERT INTO {} ({}) VALUES ({})",
        table_name, comma_sep_cols, coma_sep_values_placeholders,
    );

    let generated = quote! {
        const INSERT_QUERY: charybdis::prelude::Query = charybdis::prelude::Query::new(#query_str);
    };

    syn::parse_quote!(#generated)
}
