use quote::quote;
use syn::ImplItem;
use charybdis_parser::CharybdisArgs;

pub(crate) fn secondary_indexes_const(ch_args: &CharybdisArgs) -> ImplItem {
    let secondary_indexes: Vec<String> = ch_args.secondary_indexes.clone().unwrap_or(vec![]);

    let find_by_primary_key_query_const_str = quote! {
        const SECONDARY_INDEXES:  &'static [&'static str] = &[#(#secondary_indexes),*];
    };

    syn::parse_quote!(#find_by_primary_key_query_const_str)
}
