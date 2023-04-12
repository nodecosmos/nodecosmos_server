use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use scylla::Session;
use serde_json::to_string_pretty;
use crate::schema::{SchemaObject, SchemaObjects};

#[derive(Debug, Serialize, Deserialize)]
pub struct CurrentDbSchema {
    pub tables: SchemaObjects,
    pub udts: SchemaObjects,
    pub materialized_views: SchemaObjects,
    pub keyspace_name: String,
}

/**
 * CurrentDbSchema is a singleton that contains the current state of the database schema.
 * It is populated by the get_current_schema() function.
 * It is used to compare the current state to the desired state of the database schema.
 */
impl CurrentDbSchema {
    pub(crate) async fn new(session: &Session, keyspace_name: String)
        -> Result<CurrentDbSchema, Box<dyn std::error::Error>> {
        let mut current_schema = CurrentDbSchema {
            tables: HashMap::new(),
            udts: HashMap::new(),
            materialized_views: HashMap::new(),
            keyspace_name,
        };

        current_schema.get_tables_from_system_schema(session).await?;
        current_schema.get_udts_from_system_schema(session).await?;
        current_schema.get_materialized_views_from_system_schema(session).await?;

        return Ok(current_schema);
    }

    async fn get_tables_from_system_schema(&mut self, session: &Session)
                                           -> Result<(), Box<dyn std::error::Error>> {
        // get tables as a HashMap of column_name => column_type
        // Parse row as a single column containing an int value
        let cql = r#"
            SELECT table_name
            FROM system_schema.tables
            WHERE keyspace_name = ?
            ALLOW FILTERING
        "#;

        if let Some(rows) = session.query(cql, (self.keyspace_name.clone(),))
            .await?.rows {
            for row in rows {
                let table_name: (String,) = row.into_typed::<(String,)>()?;
                self.tables.insert(table_name.0.clone(), SchemaObject::new());
                self.populate_table_columns(&table_name.0, session).await?;
                self.populate_table_partition_keys(&table_name.0, session).await?;
                self.populate_table_clustering_keys(&table_name.0, session).await?;
                self.populate_table_secondary_indexes(&table_name.0, session).await?;
            }
        }
        return Ok(());
    }

    async fn populate_table_columns(&mut self, table_name: &String, session: &Session)
                                    -> Result<(), Box<dyn std::error::Error>> {
        // get columns and types for provided table
        let cql = r#"
            SELECT
                column_name, type
            FROM system_schema.columns
            WHERE keyspace_name = ?
            AND table_name = ?
            ALLOW FILTERING"#;

        if let Some(rows) = session.query(cql, (self.keyspace_name.clone(), &table_name))
            .await?.rows {
            for row in rows {
                let str_value: (String, String) = row.into_typed::<(String, String)>()?;
                self.tables.get_mut(table_name).unwrap().fields.insert(str_value.0, str_value.1);
            }
        }

        return Ok(());
    }

    async fn populate_table_partition_keys(&mut self, table_name: &String, session: &Session)
                                           -> Result<(), Box<dyn std::error::Error>> {
        // get partition keys for provided table
        let cql = r#"
            SELECT column_name
            FROM system_schema.columns
            WHERE keyspace_name = ?
            AND table_name = ?
            AND kind = 'partition_key'
            ALLOW FILTERING"#;

        if let Some(rows) = session.query(cql, (self.keyspace_name.clone(), &table_name))
            .await?.rows {
            for row in rows {
                let str_value: (String,) = row.into_typed::<(String,)>()?;
                self.tables.get_mut(table_name).unwrap().partition_keys.push(str_value.0);
            }
        }

        return Ok(());
    }

    async fn populate_table_clustering_keys(&mut self, table_name: &String, session: &Session)
        -> Result<(), Box<dyn std::error::Error>> {
        // get partition keys for provided table
        let cql = r#"
            SELECT column_name
            FROM system_schema.columns
            WHERE keyspace_name = ?
            AND table_name = ?
            AND kind = 'clustering'
            ALLOW FILTERING"#;

        if let Some(rows) = session.query(cql, (self.keyspace_name.clone(), &table_name))
            .await?.rows {
            for row in rows {
                let str_value: (String,) = row.into_typed::<(String,)>()?;
                self.tables.get_mut(table_name).unwrap().clustering_keys.push(str_value.0);
            }
        }

        return Ok(());
    }

    async fn populate_table_secondary_indexes(&mut self, table_name: &String, session: &Session)
        -> Result<(), Box<dyn std::error::Error>> {
        // get partition keys for provided table
        let cql = r#"
            SELECT index_name
            FROM system_schema.indexes
            WHERE keyspace_name = ?
            AND table_name = ?
            ALLOW FILTERING"#;

        if let Some(rows) = session.query(cql, (self.keyspace_name.clone(), &table_name))
            .await?.rows {
            for row in rows {
                let str_value: (String,) = row.into_typed::<(String,)>()?;
                self.tables.get_mut(table_name).unwrap().secondary_indexes.push(str_value.0);
            }
        }

        return Ok(());
    }

    async fn get_udts_from_system_schema(&mut self, session: &Session)
                                         -> Result<(), Box<dyn std::error::Error>> {
        // get tables as a HashMap of column_name => column_type
        // Parse row as a single column containing an int value
        let cql = r#"
            SELECT
                type_name,
                field_names,
                field_types
            FROM system_schema.types
            WHERE keyspace_name = ?"#;

        if let Some(rows) = session.query(cql, (self.keyspace_name.clone(),))
            .await?.rows {
            for row in rows {
                let (type_name, field_names, field_types)
                    = row.into_typed::<(String, Vec<String>, Vec<String>)>()?;

                let mut schema_object = SchemaObject::new();

                for (index, field_name) in field_names.iter().enumerate() {
                    schema_object.fields.insert(field_name.clone(), field_types[index].clone());
                }

                self.udts.insert(type_name, schema_object);
            }
        }
        return Ok(());
    }

    async fn get_materialized_views_from_system_schema(&mut self, session: &Session)
                                                       -> Result<(), Box<dyn std::error::Error>> {
        // get tables as a HashMap of column_name => column_type
        let cql = r#"
            SELECT view_name
            FROM system_schema.views
            WHERE keyspace_name = ?
            ALLOW FILTERING"#;
        if let Some(rows) = session.query(cql, (self.keyspace_name.clone(),))
            .await?.rows {
            for row in rows {
                let view_name: (String,) = row.into_typed::<(String,)>()?;
                self.materialized_views.insert(view_name.0.clone(), SchemaObject::new());
                self.populate_materialized_view_columns(&view_name.0, session).await?;
                self.populate_materialized_view_partition_key(&view_name.0, session).await?;
                self.populate_materialized_view_clustering_keys(&view_name.0, session).await?;
            }
        }
        return Ok(());
    }

    async fn populate_materialized_view_columns(&mut self, view_name: &String, session: &Session)
        -> Result<(), Box<dyn std::error::Error>> {
        // get columns and types for views
        let cql = r#"
            SELECT column_name, type
            FROM system_schema.columns
            WHERE keyspace_name = ?
            AND table_name = ?
            ALLOW FILTERING"#;
        if let Some(rows) = session.query(cql, (self.keyspace_name.clone(), &view_name))
            .await?.rows {
            for row in rows {
                let str_value: (String, String) = row.into_typed::<(String, String)>()?;
                self.materialized_views.get_mut(view_name).unwrap()
                    .fields.insert(str_value.0, str_value.1);
            }
        }

        return Ok(());
    }

    async fn populate_materialized_view_partition_key(&mut self, view_name: &String, session: &Session)
        -> Result<(), Box<dyn std::error::Error>> {
        let cql = r#"
            SELECT column_name
            FROM system_schema.columns
            WHERE keyspace_name = ?
            AND table_name = ?
            AND kind = 'partition_key'
            ALLOW FILTERING"#;

        if let Some(rows) = session.query(cql, (self.keyspace_name.clone(), &view_name))
            .await?.rows {
            for row in rows {
                let str_value: (String,) = row.into_typed::<(String,)>()?;
                self.materialized_views.get_mut(view_name).unwrap().partition_keys.push(str_value.0);
            }
        }

        return Ok(());

    }

    async fn populate_materialized_view_clustering_keys(&mut self, view_name: &String, session: &Session)
        -> Result<(), Box<dyn std::error::Error>> {
        let cql = r#"
            SELECT column_name
            FROM system_schema.columns
            WHERE keyspace_name = ?
            AND table_name = ?
            AND kind = 'clustering'
            ALLOW FILTERING"#;

        if let Some(rows) = session.query(cql, (self.keyspace_name.clone(), &view_name))
            .await?.rows {
            for row in rows {
                let str_value: (String,) = row.into_typed::<(String,)>()?;
                self.materialized_views.get_mut(view_name).unwrap()
                    .clustering_keys.push(str_value.0);
            }
        }

        return Ok(());
    }

    pub(crate) async fn get_current_schema_as_json(&self) -> Result<String, Box<dyn std::error::Error>> {
        let json = to_string_pretty(&self)?;
        return Ok(json);
    }

    pub(crate) async fn write_schema_to_json(&self) -> Result<(), Box<dyn std::error::Error>> {
        let json = self.get_current_schema_as_json().await?;
        std::fs::write("../nodecosmos/current_schema.json", json)?;
        return Ok(());
    }
}
