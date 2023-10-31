use crate::utils::serialized_field_value_adder;
use charybdis_parser::macro_args::CharybdisMacroArgs;
use proc_macro2::TokenStream;
use quote::quote;
use syn::ImplItem;

pub(crate) fn get_primary_key_values(ch_args: &CharybdisMacroArgs) -> ImplItem {
    let primary_key: Vec<String> = ch_args.get_primary_key();
    let capacity: usize = primary_key.len();

    let primary_key_accessors_tokens: TokenStream = serialized_field_value_adder(primary_key);

    let generated = quote! {
        fn get_primary_key_values(&self) -> charybdis::SerializedResult {
            let mut serialized = charybdis::SerializedValues::with_capacity(#capacity);

            #primary_key_accessors_tokens

            ::std::result::Result::Ok(::std::borrow::Cow::Owned(serialized))
        }
    };

    syn::parse_quote!(#generated)
}

pub(crate) fn get_partition_key_values(ch_args: &CharybdisMacroArgs) -> ImplItem {
    let partition_keys: Vec<String> = ch_args.partition_keys.clone().unwrap();
    let capacity: usize = partition_keys.len();

    let partition_key_accessors_tokens: TokenStream = serialized_field_value_adder(partition_keys);

    let generated: TokenStream = quote! {
        fn get_partition_key_values(&self) -> charybdis::SerializedResult {
            let mut serialized = charybdis::SerializedValues::with_capacity(#capacity);

            #partition_key_accessors_tokens

            ::std::result::Result::Ok(::std::borrow::Cow::Owned(serialized))
        }
    };

    syn::parse_quote!(#generated)
}

pub(crate) fn get_clustering_key_values(ch_args: &CharybdisMacroArgs) -> ImplItem {
    let clustering_keys = ch_args.clustering_keys.clone().unwrap();
    let capacity: usize = clustering_keys.len();

    let clustering_key_accessors_tokens: TokenStream = serialized_field_value_adder(clustering_keys);

    let generated = quote! {
        fn get_clustering_key_values(&self) -> charybdis::SerializedResult {
            let mut serialized = charybdis::SerializedValues::with_capacity(#capacity);

            #clustering_key_accessors_tokens

            ::std::result::Result::Ok(::std::borrow::Cow::Owned(serialized))
        }
    };

    syn::parse_quote!(#generated)
}
