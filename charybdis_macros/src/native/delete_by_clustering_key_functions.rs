use charybdis_parser::CharybdisArgs;
use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::{parse_str, FieldsNamed};

/// for each clustering key generate additional function that finds by partition key & partial clustering key
/// Example:
/// ```ignore
/// #[charybdis_model(
///     table_name = users,
///     partition_keys = [id],
///     clustering_keys = [org_id, created_at],
///     secondary_indexes = [])]
/// pub struct UserOps {...}
/// ```
/// we would have a functions:
/// ```ignore
///  User::delete_by_id_and_org_id(session: &Session, org_id: Uuid) -> Result<Vec<User>, CharybdisError>
///  User::delete_by_id_and_org_id_and_created_at(session: &Session, org_id: Uuid, created_at: Timestamp) -> Result<Vec<User>, CharybdisError>
pub(crate) fn delete_by_clustering_key_functions(
    ch_args: &CharybdisArgs,
    fields_named: &FieldsNamed,
    struct_name: &syn::Ident,
) -> TokenStream {
    let partition_keys = ch_args.partition_keys.clone().unwrap();
    let table_name = ch_args.table_name.clone().unwrap();

    let mut clustering_keys = ch_args.clustering_keys.clone().unwrap();

    let mut generated = quote! {};

    while clustering_keys.len() > 0 {
        let mut current_clustering_keys = clustering_keys.clone();
        let mut query_keys = partition_keys.clone();
        query_keys.append(current_clustering_keys.as_mut());

        let primary_key_where_clause: String = query_keys.join(" = ? AND ");

        let query_str = format!(
            "DELETE FROM {} WHERE {} = ?",
            table_name, primary_key_where_clause
        );

        let delete_by_fun_name_str = format!(
            "delete_by_{}",
            query_keys
                .iter()
                .map(|key| key.to_string())
                .collect::<Vec<String>>()
                .join("_and_")
        );

        let delete_by_fun_name =
            syn::Ident::new(&delete_by_fun_name_str, proc_macro2::Span::call_site());

        let arguments = query_keys
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
        let fields_str: String = query_keys
            .iter()
            .map(|key| format!("serialized.add_value(&{})?;", key))
            .collect::<Vec<String>>()
            .join("\n");

        let serialized_adder: TokenStream = parse_str(&fields_str).unwrap();

        let generated_func = quote! {
            pub async fn #delete_by_fun_name(
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

        clustering_keys.pop();

        generated.extend(generated_func);
    }

    generated
}
