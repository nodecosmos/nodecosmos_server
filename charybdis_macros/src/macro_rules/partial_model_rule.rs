use proc_macro::TokenStream;
use proc_macro2::Ident;
use quote::quote;
use syn::{Attribute, DeriveInput, FieldsNamed, parse_macro_input, parse_str, Type};
use crate::parser::parse_named_fields;

///
/// ## Generates two declarative macros for partial model usage:
/// - one is used to generate a partial model struct
/// - second one is used by the first one to get field types of provided fields
///
/// E.g.: if we have a model User with some fields, it will append partial_user! macro that
/// can be used to generate a partial model struct with only some fields of the User model.
/// It also supports same operations as the User model.
///
/// ```ignore
/// use charybdis::prelude::*;
/// use super::Address;
///
/// #[charybdis_model(table_name = "users", partition_keys = ["id"], clustering_keys = [], secondary_indexes = [])]
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
///
/// // generated macros:
///
/// #[macro_export]
/// macro_rules! partial_user {
///     ($struct_name:ident, $($field:ident),*) => {
///         #[charybdis_model(table_name = "users", partition_keys = ["id"], clustering_keys = [], secondary_indexes = [])]
///         pub struct $struct_name {
///             $(pub $field: field_type!($field),)*
///         }
///     };
/// }
///
/// // used by `partial_user!` macro to get field types
/// #[macro_export]
/// macro_rules! field_type {
///     (id) => { Uuid };
///     (username) => { Text };
///     (password) => { Text };
///     (hashed_password) => { Text };
///     (email) => { Text };
///     (created_at) => { Timestamp };
///     (updated_at) => { Timestamp };
///     (address) => { Address };
/// }
/// ```
///
///
/// ## Example usage:
///
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
pub(crate) fn partial_model_macro_generator(input: TokenStream) -> TokenStream {
    let input: DeriveInput = parse_macro_input!(input as DeriveInput);
    let charybdis_model_attr: &Attribute = input.attrs.iter()
        .find(|attr| attr.path().is_ident("charybdis_model"))
        .unwrap_or_else(|| panic!("Missing charybdis_model attribute"));


    let macro_name_str: String = format!("partial_{}", input.ident.to_string().to_lowercase());
    let macro_name: proc_macro2::TokenStream =
        parse_str::<proc_macro2::TokenStream>(&macro_name_str).unwrap();

    let fields_named: &FieldsNamed = parse_named_fields(&input);

    let mut field_types: proc_macro2::TokenStream = quote! {};
    for field in fields_named.named.iter() {
        let name: &Ident = field.ident.as_ref().unwrap();
        let ty: &Type = &field.ty;
        field_types.extend(quote! { (#name) => {#ty}; });
    };

    let expanded: proc_macro2::TokenStream = quote! {
        #input

        /// it uses field_type! macro to generate field types
        #[macro_export]
        macro_rules! #macro_name {
            ($struct_name:ident, $($field:ident),*) => {
                #charybdis_model_attr
                pub struct $struct_name {
                    $(pub $field: field_type!($field),)*
                }
            };
        }

        #[macro_export]
        macro_rules! field_type {
            #field_types
        }
    };

    println!("{}", expanded.to_string());


    TokenStream::from(expanded)
}
