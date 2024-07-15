use syn::Fields;

pub trait StructFields {
    fn struct_fields(&self) -> &Fields;
}

impl StructFields for syn::DeriveInput {
    fn struct_fields(&self) -> &Fields {
        match &self.data {
            syn::Data::Struct(data) => &data.fields,
            _ => {
                panic!("Only structs are supported");
            }
        }
    }
}
