
Phantom data is used to hold the type information for the model even though the model is not used in the struct fields.
However, it is used in `impl <T: Model>QueryBuilder<T>`that returns db result casted into the model type.

```rust
pub struct QueryBuilder<T: Model> {
    pub fields: Vec<String>,
    pub values: SerializedValues,
    pub phantom_data: std::marker::PhantomData<T>
}

impl <T: Model>QueryBuilder<T> {
    pub async fn execute(&self, session: &CachingSession) -> Result<TypedRowIter<T>, CharybdisError> {
        let result: QueryResult = session.execute(self.fields.join(" "), &self.values)
            .await.map_err(|e| CharybdisError::QueryError(e))?;

        match result.rows {
            Some(rows) => {
                let typed_rows: TypedRowIter<T> = rows.into_typed();
                Ok(typed_rows)
            }
            None => {
                Err(CharybdisError::NotFoundError(T::DB_MODEL_NAME.to_string()))
            }
        }
    }
}
```
