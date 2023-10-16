use syn::{Data, DeriveInput, Fields, FieldsNamed};

// taken from scylla_macros
pub fn parse_named_fields(input: &DeriveInput) -> &FieldsNamed {
    match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(named_fields) => named_fields,
            _ => panic!("#[charybdis_table] works only for structs with named fields!",),
        },
        _ => panic!("#[charybdis_table] works only on structs!"),
    }
}
