use std::collections::HashMap;

pub type SchemaObject = HashMap<String, String>;
pub type SchemaObjects = HashMap<String, SchemaObject>;

pub trait SchemaObjectTrait {
    fn get_cql_fields(&self) -> String;
}

impl SchemaObjectTrait for SchemaObject {
    fn get_cql_fields(&self) -> String {
        let mut cql_fields = String::new();
        for (field_name, field_type) in self.iter() {
            cql_fields.push_str(&format!("{} {}, ", field_name, field_type));
        }
        cql_fields.pop();
        cql_fields.pop();
        cql_fields
    }
}
