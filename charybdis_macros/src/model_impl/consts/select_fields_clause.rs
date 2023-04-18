use crate::helpers::comma_sep_cols;
use charybdis_parser::CharybdisArgs;
use quote::quote;
use syn::{FieldsNamed, ImplItem};

pub fn select_fields_clause(ch_args: &CharybdisArgs, fields_named: &FieldsNamed) -> ImplItem {
    let table_name = ch_args.table_name.clone().unwrap();

    let comma_sep_cols = comma_sep_cols(fields_named);

    let query_str = format!("SELECT {} FROM {}", comma_sep_cols, table_name.to_string());

    let generated = quote! {
        const SELECT_FIELDS_CLAUSE: &'static str = #query_str;
    };

    syn::parse_quote!(#generated)
}
