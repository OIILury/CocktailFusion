use chrono::NaiveDateTime;

#[derive(Debug)]
pub struct AuthenticatedUser {
  pub niveau: i64,
  pub last_login_datetime: NaiveDateTime,
  pub user_id: String,
}
