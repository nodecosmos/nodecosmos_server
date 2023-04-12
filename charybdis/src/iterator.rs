// use crate::errors::CharybdisError;
// use crate::model::Model;
// use crate::prelude::Row;
// 
// pub struct CharTypedRowIter<RowT: Model> {
//     row_iter: std::vec::IntoIter<Row>,
//     phantom_data: std::marker::PhantomData<RowT>,
// }
// 
// impl<RowT: Model> Iterator for CharTypedRowIter<RowT> {
//     type Item = Result<RowT, CharybdisError>;
// 
//     fn next(&mut self) -> Option<Result<RowT, CharybdisError>> {
//         self.row_iter.next().map(|row| RowT::from_row(&row))
//     }
// }
// 
// pub trait IntoCharTypedRows {
//     fn into_typed<RowT: Model>(self) -> CharTypedRowIter<RowT>;
// }
// 
// impl IntoCharTypedRows for Vec<Row> {
//     fn into_typed<RowT: Model>(self) -> CharTypedRowIter<RowT> {
//         CharTypedRowIter {
//             row_iter: self.into_iter(),
//             phantom_data: Default::default(),
//         }
//     }
// }
