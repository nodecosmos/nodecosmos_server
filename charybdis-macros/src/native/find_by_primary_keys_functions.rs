use crate::utils::{comma_sep_cols, serialized_value_adder, struct_fields_to_fn_args};
use charybdis_parser::macro_args::CharybdisMacroArgs;
use proc_macro2::TokenStream;
use quote::quote;
use syn::Field;

const MAX_FIND_BY_FUNCTIONS: usize = 3;

/// for each key in the clustering key, and for complete partition key, generate a function that
/// finds a model by that keys in order defined.
/// Scylla enables us to query by complete partition key and partial clustering key.
pub(crate) fn find_by_primary_keys_functions(
    ch_args: &CharybdisMacroArgs,
    fields: &Vec<Field>,
    struct_name: &syn::Ident,
) -> TokenStream {
    let partition_keys = ch_args.partition_keys.clone().unwrap();
    let table_name = ch_args.table_name.clone().unwrap();
    let comma_sep_cols = comma_sep_cols(fields);

    let mut primary_key = ch_args.primary_key();
    let mut generated = quote! {};

    let mut i = 0;

    while primary_key.len() >= partition_keys.len() {
        if i > MAX_FIND_BY_FUNCTIONS {
            break;
        }

        i += 1;

        let current_keys = primary_key.clone();
        let primary_key_where_clause: String = current_keys.join(" = ? AND ");
        let query_str = format!(
            "SELECT {} FROM {} WHERE {} = ?",
            comma_sep_cols, table_name, primary_key_where_clause
        );
        let find_by_fun_name_str = format!(
            "find_by_{}",
            current_keys
                .iter()
                .map(|key| key.to_string())
                .collect::<Vec<String>>()
                .join("_and_")
        );
        let find_by_fun_name = syn::Ident::new(&find_by_fun_name_str, proc_macro2::Span::call_site());

        let arguments = struct_fields_to_fn_args(struct_name.to_string(), fields.clone(), current_keys.clone());
        let capacity = current_keys.len();
        let serialized_adder = serialized_value_adder(current_keys.clone());

        let generated_func = quote! {
            pub async fn #find_by_fun_name(
                session: &charybdis::CachingSession,
                #(#arguments),*
            ) -> Result<charybdis::stream::CharybdisModelStream<#struct_name>, charybdis::errors::CharybdisError> {
                use futures::TryStreamExt;

                let mut serialized = charybdis::SerializedValues::with_capacity(#capacity);

                #serialized_adder

                let query_result = session.execute_iter(#query_str, serialized).await?;
                let rows = query_result.into_typed::<Self>();

                Ok(charybdis::stream::CharybdisModelStream::from(rows))
            }
        };

        primary_key.pop();

        generated.extend(generated_func);
    }

    generated
}
