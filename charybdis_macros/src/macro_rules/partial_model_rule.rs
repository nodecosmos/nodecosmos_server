use charybdis_parser::{parse_named_fields, CharybdisArgs};
use proc_macro::TokenStream;
use proc_macro2::Ident;
use quote::quote;
use syn::{parse_macro_input, parse_str, DeriveInput, FieldsNamed, Type};

///
/// ## Generates two declarative macros for partial model usage:
/// - one is used to generate a partial model struct
/// - second one is used by the first one to get field types of provided fields
///
/// E.g.: if we have a model User with some fields, it will append partial_<model_name>! macro that
/// can be used to generate a partial model struct with only some fields that can be used for ORM.
/// It is basically the same as base model struct, but with provided fields only.
///
/// So, if we have a model User with some fields:
/// ```ignore
/// use charybdis::prelude::*;
/// use super::Address;
/// #[partial_model_generator]
/// #[charybdis_model(table_name = "users",
///                   partition_keys = ["id"],
///                   clustering_keys = []
///                   secondary_indexes = [])]
/// pub struct User {
///     pub id: Uuid,
///     pub username: Text,
///     pub password: Text,
///     pub hashed_password: Text,
///     pub email: Text,
///     pub created_at: Timestamp,
///     pub updated_at: Timestamp,
///     pub address: Address,
/// }
/// ```
/// It will generate a `partial_user!` macro that can be used to generate a partial users structs.
///
/// ## Example generation:
/// ```ignore
/// partial_user!(PartialUser, id, username, email);
/// ```
/// It will generate a struct with only those fields:
/// ```ignore
/// #[charybdis_model(table_name = "users",
///                   partition_keys = ["id"],
///                   clustering_keys = []
///                   secondary_indexes = [])]
/// pub struct PartialUser {
///    pub id: Uuid,
///    pub username: Text,
///    pub email: Text,
/// }
///```
/// And we can use it as a normal model struct.
///
/// ```ignore
/// let mut partial_user = PartialUser {id, username, email};
/// partial_user.insert(&session).await?;
/// partial_user.find_by_id(&session).await?;
/// partial_user.find_by_id_and_email(&session).await?;
/// ```
///
/// ## Example usage:
/// ```rust
///     partial_user!(PartialUser, id, username, email);
///
///     let mut partial_user = PartialUser {
///         id: Uuid::new_v4(),
///         username: "test".to_string(),
///         email: "test@gmail.com".to_string(),
///     };
///
///    println!("{:?}", partial_user);
///```
///---
///
/// ### `#[charybdis_model]` declaration
/// It also appends `#[charybdis_model(...)]` declaration with clustering keys and secondary indexes
/// based on fields that are provided in partial_model struct.
///
/// E.g. if we have model:
/// ```ignore
/// #[partial_model_generator]
/// #[charybdis_model(table_name = "users",
///                   partition_keys = ["id"],
///                   clustering_keys = ["created_at", "updated_at"],
///                   secondary_indexes = [])]
/// pub struct User {
///     pub id: Uuid,
///     pub username: Text,
///     pub password: Text,
///     pub hashed_password: Text,
///     pub email: Text,
///     pub created_at: Timestamp,
///     pub updated_at: Timestamp,
/// }
/// ```
///
/// and we use partial model macro:
/// ```ignore
/// partial_user!(UserOps, id, username, email, created_at);
/// ```
/// it will generate a struct with `#[charybdis_model(...)]` declaration:
///
/// ```ignore
/// #[charybdis_model(table_name = "users",
///                   partition_keys = ["id"],
///                   clustering_keys = ["created_at"],
///                   secondary_indexes = [])]
/// pub struct UserOps {...}
/// ```
/// Note that `updated_at` is not present in generated declaration.
/// However, all partition keys are required for db operations, so we can't have partial partition keys.
///

pub(crate) fn partial_model_macro_generator(input: TokenStream) -> TokenStream {
    let input: DeriveInput = parse_macro_input!(input as DeriveInput);
    let fields_named: &FieldsNamed = parse_named_fields(&input);

    // TODO: for models without all clustering keys, this will panic!
    //       we should add support for partial clustering keys.

    // TODO: we enable better error handling for this macro as it hard to debug
    //       nested macros when they panic.

    // macro names (avoiding name collisions)
    let macro_name_str: String = format!("partial_{}", input.ident.to_string().to_lowercase());
    let macro_name: proc_macro2::TokenStream =
        parse_str::<proc_macro2::TokenStream>(&macro_name_str).unwrap();

    let field_type_macro_name_str: String = format!("{}_field_type", macro_name_str);

    let field_type_macro_name: Ident =
        Ident::new(&field_type_macro_name_str, proc_macro2::Span::call_site());

    // macro that generates field types
    let field_type_macro_body: proc_macro2::TokenStream = build_field_type_macro_body(fields_named);

    let char_args: CharybdisArgs = CharybdisArgs::from_derive(&input);

    let table_name = char_args.table_name.unwrap();

    let cks: Vec<String> = char_args.clustering_keys.unwrap_or(vec![]);
    let pks: Vec<String> = char_args.partition_keys.unwrap_or(vec![]);
    let sec_idxes: Vec<String> = char_args.secondary_indexes.unwrap_or(vec![]);

    let cks: proc_macro2::TokenStream = parse_str(&format!("{:?}", cks)).unwrap();
    let pks: proc_macro2::TokenStream = parse_str(&format!("{:?}", pks)).unwrap();
    let sec_idxes: proc_macro2::TokenStream = parse_str(&format!("{:?}", sec_idxes)).unwrap();

    let expanded: proc_macro2::TokenStream = quote! {
        #input

         #[allow(unused_macros)]
        macro_rules! #macro_name {
            ($struct_name:ident, $($field:ident),*) => {
                #[charybdis_model(table_name=#table_name,
                                  partition_keys=#pks,
                                  clustering_keys=#cks,
                                  secondary_indexes=#sec_idxes)]
                pub struct $struct_name {
                    $(pub $field: #field_type_macro_name!($field),)*
                }
            };
        }

        pub(crate) use #macro_name;

         #[allow(unused_macros)]
        macro_rules! #field_type_macro_name {
            #field_type_macro_body
        }

        pub(crate) use #field_type_macro_name;
    };

    TokenStream::from(expanded)
}

/// Builds field_type! macro body that is used by partial model macro to get field types
fn build_field_type_macro_body(fields_named: &FieldsNamed) -> proc_macro2::TokenStream {
    let mut field_types: proc_macro2::TokenStream = quote! {};
    for field in fields_named.named.iter() {
        let name: &Ident = field.ident.as_ref().unwrap();
        let ty: &Type = &field.ty;
        field_types.extend(quote! { (#name) => {#ty}; });
    }
    field_types
}
