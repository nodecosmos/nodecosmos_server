use crate::helpers::{camel_to_snake_case, comma_sep_cols};
use charybdis_parser::CharybdisArgs;
use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::{parse_str, FieldsNamed};

pub fn find_model_query_rule(
    args: &CharybdisArgs,
    fields_named: &FieldsNamed,
    struct_name: &Ident,
) -> TokenStream {
    let comma_sep_cols = comma_sep_cols(fields_named);
    let table_name = args.table_name.clone().unwrap();
    let struct_name_str = camel_to_snake_case(&struct_name.to_string());

    let macro_name_str: String = format!("find_{}_query", struct_name_str);
    let macro_name: TokenStream = parse_str::<TokenStream>(&macro_name_str).unwrap();

    let query_str = format!(
        "SELECT {} FROM {} WHERE ",
        comma_sep_cols,
        table_name.to_string()
    );

    let expanded = quote! {
        #[allow(unused_macros)]
        macro_rules! #macro_name {
            ($query: literal) => {
                let query_str = concat!(#query_str, $query)
                charybdis::prelude::Query::new(query_str)
            }
        }

        pub(crate) use #macro_name;
    };

    TokenStream::from(expanded)
}

pub fn find_model_iter_query_rule(
    args: &CharybdisArgs,
    fields_named: &FieldsNamed,
    struct_name: &Ident,
) -> TokenStream {
    let comma_sep_cols = comma_sep_cols(fields_named);
    let table_name = args.table_name.clone().unwrap();
    let struct_name_str = camel_to_snake_case(&struct_name.to_string());

    let macro_name_str: String = format!("find_{}_iter_query", struct_name_str);
    let macro_name: TokenStream = parse_str::<TokenStream>(&macro_name_str).unwrap();

    let query_str = format!(
        "SELECT {} FROM {} WHERE ",
        comma_sep_cols,
        table_name.to_string()
    );

    let expanded = quote! {
        #[allow(unused_macros)]
        macro_rules! #macro_name {
            ($query: literal, $page_size: u32) => {
                let query_str = concat!(#query_str, $query)
                let query = charybdis::prelude::Query::new(query_str)
                query.set_page_size($page_size)
            }
        }

        pub(crate) use #macro_name;
    };

    TokenStream::from(expanded)
}
