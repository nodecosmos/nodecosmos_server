use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use scylla::Session;
use serde_json::to_string_pretty;
use crate::schema::SchemaObjects;

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct CurrentDbSchema {
    pub tables: SchemaObjects,
    pub udts: SchemaObjects,
    pub materialized_views: SchemaObjects,
}

/**
 * CurrentDbSchema is a singleton that contains the current state of the database schema.
 * It is populated by the get_current_schema() function.
 * It is used to compare the current state to the desired state of the database schema.
 */
impl CurrentDbSchema {
    pub async fn new(session: &Session) -> Result<CurrentDbSchema, Box<dyn std::error::Error>> {
        let mut current_schema = CurrentDbSchema {
            tables: HashMap::new(),
            udts: HashMap::new(),
            materialized_views: HashMap::new(),
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
            WHERE keyspace_name = 'nodecosmos'
        "#;

        if let Some(rows) = session
            .query(cql, &[]).await?.rows {
            for row in rows {
                let table_name: (String,) = row.into_typed::<(String,)>()?;
                self.tables.insert(table_name.0.clone(), HashMap::new());
                self.populate_table_columns(&table_name.0, session).await?;
            }
        }
        return Ok(());
    }


    async fn populate_table_columns(&mut self, table_name: &String, session: &Session)
        -> Result<(), Box<dyn std::error::Error>> {
        // get columns and types for tables without views
        let cql = r#"
            SELECT column_name, type
            FROM system_schema.columns
            WHERE keyspace_name = 'nodecosmos'
            AND table_name = ?"#;

        if let Some(rows) = session
            .query(cql, (table_name,)).await?.rows {
            for row in rows {
                let str_value: (String, String) = row.into_typed::<(String, String)>()?;
                self.tables.get_mut(table_name).unwrap().insert(str_value.0, str_value.1);
            }
        }

        return Ok(());
    }

    async fn get_udts_from_system_schema(&mut self, session: &Session)
        -> Result<(), Box<dyn std::error::Error>> {
        // get tables as a HashMap of column_name => column_type
        // Parse row as a single column containing an int value
        let cql = r#"
            SELECT type_name
            FROM system_schema.types
            WHERE keyspace_name = 'nodecosmos'"#;

        if let Some(rows) = session
            .query(cql, &[]).await?.rows {
            for row in rows {
                let type_name: (String,) = row.into_typed::<(String,)>()?;
                self.udts.insert(type_name.0.clone(), HashMap::new());
                self.populate_udt_columns(&type_name.0, session).await?;
            }
        }
        return Ok(());
    }

    async fn populate_udt_columns(&mut self, type_name: &String, session: &Session)
        -> Result<(), Box<dyn std::error::Error>> {
        // get columns and types for udt
        let cql = r#"
            SELECT field_name, type
            FROM system_schema.columns
            WHERE keyspace_name = 'nodecosmos'
            AND type_name = ?"#;
        if let Some(rows) = session
            .query(cql, (type_name,)).await?.rows {
            for row in rows {
                let str_value: (String, String) = row.into_typed::<(String, String)>()?;
                self.udts.get_mut(type_name).unwrap().insert(str_value.0, str_value.1);
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
            WHERE keyspace_name = 'nodecosmos'"#;
        if let Some(rows) = session
            .query(cql, &[]).await?.rows {
            for row in rows {
                let view_name: (String,) = row.into_typed::<(String,)>()?;
                self.materialized_views.insert(view_name.0.clone(), HashMap::new());
                self.populate_materialized_view_columns(&view_name.0, session).await?;
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
            WHERE keyspace_name = 'nodecosmos'
            AND table_name = ?"#;
        if let Some(rows) = session
            .query(cql, (view_name,)).await?.rows {
            for row in rows {
                let str_value: (String, String) = row.into_typed::<(String, String)>()?;
                self.materialized_views.get_mut(view_name).unwrap()
                    .insert(str_value.0, str_value.1);
            }
        }

        return Ok(());
    }

    pub async fn get_current_schema_as_json(&self) -> Result<String, Box<dyn std::error::Error>> {
        let json = to_string_pretty(&self)?;
        return Ok(json);
    }

    pub async fn get_current_schema_as_json_file(&self) -> Result<(), Box<dyn std::error::Error>> {
        let json = to_string_pretty(&self)?;
        std::fs::write("../../../current_schema.json", json)?;
        return Ok(());
    }
}
