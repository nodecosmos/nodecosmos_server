use crate::model::Model;

pub trait InsertJson {
    fn insert_json(&self, json: String);
}

impl <T: Default + Model> InsertJson for T {
    fn insert_json(&self, json: String) {
        println!("INSERT INTO {} JSON {}", T::DB_MODEL_NAME, json)
    }
}
