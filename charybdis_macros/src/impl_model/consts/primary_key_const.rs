use quote::quote;
use syn::{ImplItem};
use crate::parser::{CharybdisArgs};

pub(crate) fn primary_key_const(ch_args: &CharybdisArgs) -> ImplItem {
    let mut primary_key = ch_args.partition_keys.clone().unwrap();
    let mut clustering_keys = ch_args.clustering_keys.clone().unwrap();

    primary_key.append(clustering_keys.as_mut());

    let primary_key_const = quote! {
        const PRIMARY_KEY: &'static [&'static str] = &[#(#primary_key),*];
    };

    syn::parse_quote!(#primary_key_const)
}

