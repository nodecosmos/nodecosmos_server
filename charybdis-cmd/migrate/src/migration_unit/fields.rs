use crate::migration_unit::runner::INDEX_SUFFIX;
use crate::migration_unit::MigrationUnitData;
use colored::Colorize;

pub type FieldName = String;
pub type FieldType = String;
type NewField = (FieldName, FieldType);

pub struct MigrationUnitFields<'a> {
    data: &'a MigrationUnitData<'a>,
    pub(crate) new_fields: Vec<NewField>,
    pub(crate) removed_fields: Vec<FieldName>,
    pub(crate) new_secondary_indexes: Vec<FieldName>,
    pub(crate) removed_secondary_indexes: Vec<FieldName>,
}

impl<'a> MigrationUnitFields<'a> {
    pub(crate) fn new(data: &'a MigrationUnitData) -> Self {
        let mut fields = Self {
            data,
            new_fields: vec![],
            removed_fields: vec![],
            new_secondary_indexes: vec![],
            removed_secondary_indexes: vec![],
        };

        fields.fetch_new_fields();
        fields.fetch_removed_fields();
        fields.fetch_new_secondary_indexes();
        fields.fetch_removed_secondary_indexes();

        fields
    }

    pub(crate) fn construct_index_name(&self, column_name: &String) -> String {
        format!("{}_{}_{}", self.data.migration_object_name, column_name, INDEX_SUFFIX)
    }

    // Checks if any field of db schema has changed type in code schema.
    pub(crate) fn field_type_changed(&self) -> bool {
        for (field_name, field_type) in self.data.current_code_schema.fields.iter() {
            if let Some(db_field_type) = self.data.current_db_schema.fields.get(field_name) {
                let code_field_type = field_type.to_lowercase().replace(' ', "");
                let db_field_type = db_field_type.to_lowercase().replace(' ', "");

                if code_field_type != db_field_type {
                    println!(
                        "\nType Change: {} -> {}",
                        db_field_type.to_uppercase().yellow().bold(),
                        code_field_type.to_uppercase().bright_red().bold()
                    );

                    return true;
                }
            }
        }

        false
    }

    pub(crate) fn partition_key_changed(&self) -> bool {
        let mut code_partition_keys: Vec<String> = self.data.current_code_schema.partition_keys.clone();
        let mut db_partition_keys = self.data.current_db_schema.partition_keys.clone();

        code_partition_keys.sort();
        db_partition_keys.sort();

        code_partition_keys != db_partition_keys
    }

    pub(crate) fn clustering_key_changed(&self) -> bool {
        let mut code_clustering_keys = self.data.current_code_schema.clustering_keys.clone();
        let mut db_clustering_keys = self.data.current_db_schema.clustering_keys.clone();

        code_clustering_keys.sort();
        db_clustering_keys.sort();

        code_clustering_keys != db_clustering_keys
    }

    fn fetch_new_fields(&mut self) {
        for (field_name, field_type) in self.data.current_code_schema.fields.iter() {
            if !self.data.current_db_schema.fields.contains_key(field_name) {
                self.new_fields.push((field_name.clone(), field_type.clone()));
            }
        }
    }

    fn fetch_removed_fields(&mut self) {
        for (field_name, _) in self.data.current_db_schema.fields.iter() {
            if !self.data.current_code_schema.fields.contains_key(field_name) {
                self.removed_fields.push(field_name.clone());
            }
        }
    }

    fn fetch_new_secondary_indexes(&mut self) {
        self.data
            .current_code_schema
            .secondary_indexes
            .iter()
            .for_each(|sec_index_column| {
                let index_name: String = self.construct_index_name(sec_index_column);

                if !self.data.current_db_schema.secondary_indexes.contains(&index_name) {
                    self.new_secondary_indexes.push(sec_index_column.clone());
                }
            });
    }

    fn fetch_removed_secondary_indexes(&mut self) {
        let code_sec_indexes: Vec<String> = self
            .data
            .current_code_schema
            .secondary_indexes
            .iter()
            .map(|sec_idx_col| self.construct_index_name(sec_idx_col))
            .collect();

        self.data.current_db_schema.secondary_indexes.iter().for_each(|index| {
            if !code_sec_indexes.contains(index) {
                self.removed_secondary_indexes.push(index.clone());
            }
        });
    }
}
