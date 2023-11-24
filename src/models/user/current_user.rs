use crate::models::user::CurrentUser;

impl CurrentUser {
    pub fn full_name(&self) -> String {
        format!("{} {}", self.first_name, self.last_name)
    }
}
