use quote::quote;
use syn::ImplItem;
use charybdis_parser::CharybdisArgs;

pub(crate) fn partition_keys_const(ch_args: &CharybdisArgs) -> ImplItem {
    let partition_keys = ch_args.partition_keys.as_ref().unwrap_or_else(|| {
        panic!(r#"
        The `partition_keys` attribute is required for the `charybdis_model` macro.
        Please provide a list of partition keys for the model.
        e.g. #[charybdis_model(partition_keys = ["id"])]
        "#)
    });

    let partition_keys = partition_keys.iter().map(|pk| quote!(#pk));

    let find_by_primary_key_query_const_str = quote! {
        const PARTITION_KEYS: &'static [&'static str] = &[#(#partition_keys),*];
    };

    syn::parse_quote!(#find_by_primary_key_query_const_str)
}
