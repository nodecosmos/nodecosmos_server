use crate::helpers::comma_sep_cols;
use charybdis_parser::CharybdisArgs;
use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::{parse_str, FieldsNamed};

/// for each key in the clustering key, and for complete partition key, generate a function that
/// finds a model by that keys in order defined.
/// Scylla enables us to query by complete partition key and partial clustering key.
pub(crate) fn find_by_primary_keys_functions(
    ch_args: &CharybdisArgs,
    fields_named: &FieldsNamed,
    struct_name: &syn::Ident,
) -> TokenStream {
    let partition_keys = ch_args.partition_keys.clone().unwrap();
    let table_name = ch_args.table_name.clone().unwrap();
    let comma_sep_cols = comma_sep_cols(fields_named);

    let mut primary_key = ch_args.get_primary_key();
    let mut generated = quote! {};

    while primary_key.len() >= partition_keys.len() {
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

        let find_by_fun_name =
            syn::Ident::new(&find_by_fun_name_str, proc_macro2::Span::call_site());

        let arguments = current_keys
            .iter()
            .map(|key| {
                let key_type = fields_named
                    .named
                    .iter()
                    .find(|field| field.ident.as_ref().unwrap() == key)
                    .unwrap()
                    .ty
                    .clone();

                parse_str::<syn::FnArg>(&format!("{}: {}", key, key_type.to_token_stream()))
                    .unwrap()
            })
            .collect::<Vec<syn::FnArg>>();

        let capacity = arguments.len();
        let fields_str: String = current_keys
            .iter()
            .map(|key| format!("serialized.add_value(&{})?;", key))
            .collect::<Vec<String>>()
            .join("\n");

        let serialized_adder: TokenStream = parse_str(&fields_str).unwrap();

        let generated_func = quote! {
            pub async fn #find_by_fun_name(
                session: &charybdis::CachingSession,
                #(#arguments),*
            ) -> Result<charybdis::CharybdisModelStream<#struct_name>, charybdis::CharybdisError> {
                use futures::TryStreamExt;

                let mut serialized = charybdis::SerializedValues::with_capacity(#capacity);

                #serialized_adder

                let query_result = session.execute_iter(#query_str, serialized).await?;
                let rows = query_result.into_typed::<Self>();

                Ok(charybdis::CharybdisModelStream::from(rows))
            }
        };

        primary_key.pop();

        generated.extend(generated_func);
    }

    generated
}
