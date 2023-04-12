use quote::quote;
use syn::ImplItem;
use charybdis_parser::CharybdisArgs;

pub(crate) fn clustering_keys_const(ch_args: &CharybdisArgs) -> ImplItem {
    let clustering_keys = ch_args.clustering_keys.clone().unwrap_or(vec![]);

    let generated = quote! {
        const CLUSTERING_KEYS:  &'static [&'static str] = &[#(#clustering_keys),*];
    };

    syn::parse_quote!(#generated)
}
