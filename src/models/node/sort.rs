use crate::models::node::Node;

pub trait SortNodes {
    fn sort_by_depth(&mut self);
}

impl SortNodes for Vec<Node> {
    fn sort_by_depth(&mut self) {
        self.sort_by(|a, b| {
            let a_len = a.ancestor_ids.as_ref().map_or(0, |ids| ids.len());
            let b_len = b.ancestor_ids.as_ref().map_or(0, |ids| ids.len());

            a_len.cmp(&b_len)
        });
    }
}
