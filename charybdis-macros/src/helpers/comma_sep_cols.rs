use syn::FieldsNamed;

pub(crate) fn comma_sep_cols(fields_named: &FieldsNamed) -> String {
    fields_named
        .named
        .iter()
        .map(|field| field.ident.as_ref().unwrap().to_string())
        .collect::<Vec<String>>()
        .join(", ")
}
