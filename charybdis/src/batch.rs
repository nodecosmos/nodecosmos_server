use crate::{CharybdisError, Model, SerializedValues, ValueList};
use scylla::transport::session::TypedRowIter;
use scylla::CachingSession;

// Simple batch for Charybdis models
pub struct CharybdisModelBatch {
    batch: scylla::batch::Batch,
    values: Vec<SerializedValues>,
    with_uniq_timestamp: bool,
    current_timestamp: i64,
}

impl CharybdisModelBatch {
    pub fn new() -> Self {
        let now = chrono::Utc::now().timestamp_micros();

        Self {
            batch: scylla::batch::Batch::default(),
            values: Vec::new(),
            with_uniq_timestamp: false,
            current_timestamp: now,
        }
    }

    pub fn with_uniq_timestamp(mut self) -> Self {
        self.with_uniq_timestamp = true;
        self
    }

    fn inject_timestamp(&mut self, statement: &str) -> String {
        return if statement.contains("SET") {
            // insert timestamp before SET
            let mut parts = statement.split("SET");
            let first_part = parts.next().unwrap();
            let second_part = parts.next().unwrap();
            format!(
                "{} USING TIMESTAMP {} SET{}",
                first_part, self.current_timestamp, second_part
            )
        } else if statement.contains("DELETE") {
            // insert timestamp before WHERE
            let mut parts = statement.split("WHERE");
            let first_part = parts.next().unwrap();
            let second_part = parts.next().unwrap();

            format!(
                "{} USING TIMESTAMP {} WHERE{}",
                first_part, self.current_timestamp, second_part
            )
        } else {
            // append timestamp to the end
            format!("{} USING TIMESTAMP {}", statement, self.current_timestamp)
        };
    }

    fn append_statement_to_batch(&mut self, statement: &str) {
        let mut query = statement.to_string();

        if self.with_uniq_timestamp {
            query = self.inject_timestamp(statement);
            self.current_timestamp += 1;
        }

        self.batch.append_statement(query.as_str());
    }

    pub fn append_create<T: Model + ValueList>(&mut self, model: &T) -> Result<(), CharybdisError> {
        self.append_statement_to_batch(T::INSERT_QUERY);
        let values = model.serialized()?;

        self.values.push(values.into_owned());

        Ok(())
    }

    pub fn append_creates<T: Model + ValueList>(
        &mut self,
        iter: TypedRowIter<T>,
    ) -> Result<(), CharybdisError> {
        for model in iter {
            match model {
                Ok(model) => {
                    let result = self.append_create(&model);
                    result?
                }
                Err(e) => return Err(CharybdisError::from(e)),
            };
        }

        Ok(())
    }

    pub fn append_update<T: Model>(&mut self, model: &T) -> Result<(), CharybdisError> {
        self.append_statement_to_batch(T::UPDATE_QUERY);

        let update_values = model
            .get_update_values()
            .map_err(|e| CharybdisError::SerializeValuesError(e, T::DB_MODEL_NAME.to_string()))?;

        self.values.push(update_values.into_owned());

        Ok(())
    }

    pub fn append_updates<T: Model + ValueList>(
        &mut self,
        iter: TypedRowIter<T>,
    ) -> Result<(), CharybdisError> {
        for model in iter {
            match model {
                Ok(model) => {
                    let result = self.append_update(&model);
                    result?
                }
                Err(e) => return Err(CharybdisError::from(e)),
            };
        }

        Ok(())
    }

    pub fn append_delete<T: Model + ValueList>(&mut self, model: &T) -> Result<(), CharybdisError> {
        self.append_statement_to_batch(T::DELETE_QUERY);

        let primary_key_values = model
            .get_primary_key_values()
            .map_err(|e| CharybdisError::SerializeValuesError(e, T::DB_MODEL_NAME.to_string()))?;

        self.values.push(primary_key_values.into_owned());

        Ok(())
    }

    pub fn append_deletes<T: Model + ValueList>(
        &mut self,
        iter: TypedRowIter<T>,
    ) -> Result<(), CharybdisError> {
        for model in iter {
            match model {
                Ok(model) => {
                    let result = self.append_delete(&model);
                    result?
                }
                Err(e) => return Err(CharybdisError::from(e)),
            };
        }

        Ok(())
    }

    pub fn append_statement(
        &mut self,
        statement: &str,
        values: impl ValueList,
    ) -> Result<(), CharybdisError> {
        self.append_statement_to_batch(statement);

        let values = values.serialized()?;
        self.values.push(values.into_owned());

        Ok(())
    }

    pub async fn execute(&self, db_session: &CachingSession) -> Result<(), CharybdisError> {
        db_session.batch(&self.batch, self.values.clone()).await?;

        Ok(())
    }
}

impl Default for CharybdisModelBatch {
    fn default() -> Self {
        Self::new()
    }
}
