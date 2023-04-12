use quote::quote;
use syn::ImplItem;
use charybdis_parser::CharybdisArgs;

pub(crate) fn db_model_name_const(ch_args: &CharybdisArgs) -> ImplItem {
    let table_name = ch_args.table_name.as_ref().unwrap();

    let generated = quote! {
        const DB_MODEL_NAME: &'static str = #table_name;
    };

    syn::parse_quote!(#generated)
}
