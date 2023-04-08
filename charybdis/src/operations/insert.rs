use scylla::_macro_internal::ValueList;
use scylla::Session;
use scylla::transport::errors::QueryError;
use crate::model::Model;

pub trait Insert
{
    async fn insert(&self, session: &Session) -> Result<(), QueryError>;
}


// From Scylla docs:
// Prepared queries have good performance, much better than simple queries.
// By default they use shard/token aware load balancing.
// Always pass partition key values as bound values.
// Otherwise the driver can’t hash them to compute partition key and they will be sent
// to the wrong node, which worsens performance.
impl <T:  Model + ValueList> Insert  for T {
    async fn insert(&self, _session: &Session) -> Result<(), QueryError> {
        // let query_str: String = format!(
        //     "INSERT INTO {} ({}) VALUES (?)",
        //     T::DB_MODEL_NAME,
        //     T::filed_names().join(",")
        // );
        //
        // let prepared_statement: PreparedStatement = session.prepare(query_str.as_str()).await?;
        // let result: QueryResult = session.execute(&prepared_statement, self).await?;
        // let mut rows: Vec<Row> = result.rows().unwrap();
        // let row: Row = rows.pop().unwrap();
        //
        Ok(())
    }
}
