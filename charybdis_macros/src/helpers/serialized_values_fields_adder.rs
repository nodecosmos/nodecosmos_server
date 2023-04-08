use proc_macro2::TokenStream;
use syn::parse_str;

pub(crate) fn serialized_values_fields_adder(fields: Vec<String>) -> TokenStream {
    let fields_str: String = fields
        .iter()
        .map(|key| format!("serialized.add_value(&self.{});", key))
        .collect::<Vec<String>>()
        .join("\n");

    parse_str(&fields_str).unwrap()
}
