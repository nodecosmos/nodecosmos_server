use crate::helpers::serialized_field_value_adder;
use charybdis_parser::CharybdisArgs;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{FieldsNamed, ImplItem};

/// (check update_query_const.rs)
///
/// First we get all the non primary key fields used in set_fields clause then we get all primary key fields
/// used in where clause.
pub(crate) fn get_update_values(ch_args: &CharybdisArgs, fields_named: &FieldsNamed) -> ImplItem {
    let mut primary_key = ch_args.get_primary_key();

    let mut update_values: Vec<String> = fields_named
        .named
        .iter()
        .map(|field| field.ident.as_ref().unwrap().to_string())
        .filter(|field| !primary_key.contains(field))
        .collect();

    update_values.append(primary_key.as_mut());

    let capacity: usize = update_values.len();
    let serialized_field_value_adder: TokenStream = serialized_field_value_adder(update_values);

    let generated = quote! {
        fn get_update_values(&self) -> charybdis::SerializedResult {
            let mut serialized = charybdis::SerializedValues::with_capacity(#capacity);

            #serialized_field_value_adder

            ::std::result::Result::Ok(::std::borrow::Cow::Owned(serialized))
        }
    };

    syn::parse_quote!(#generated)
}
