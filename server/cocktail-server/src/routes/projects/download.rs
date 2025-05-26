use crate::{error::WebError, models::auth::AuthenticatedUser, routes::paths::DownloadProject};
use axum::{extract::State, response::IntoResponse};
use cocktail_db_web::WebDatabase;
use hyper::header;

pub async fn download_project(
  DownloadProject { project_id }: DownloadProject,
  AuthenticatedUser {
    niveau: _,
    last_login_datetime: _,
    user_id,
  }: AuthenticatedUser,
  State(db): State<WebDatabase>,
) -> Result<impl IntoResponse, WebError> {
  let directory_path = format!("project-data/{}", project_id.to_string());
  let project = cocktail_db_web::project(&db, project_id.to_hyphenated(), &user_id).await?;

  let index = fts::Index::open_in_dir(directory_path)?;

  if project.tweets_count > 5000 {
    return Err(WebError::Community(
      "Nombre de tweets trop élevé".to_owned(),
    ));
  }
  let tweets = fts::get_all_tweets(&index)?;

  let mut content = "".to_owned();
  content.push_str(
    &vec![
      "ID".to_owned(),
      "Publication Date".to_owned(),
      "Direct Link".to_owned(),
      "Content Publisher".to_owned(),
      "Name Publisher".to_owned(),
      "Username Media URL".to_owned(),
      "Nombre Réponses".to_owned(),
      "Nombre RT".to_owned(),
      "Nombre Citations".to_owned(),
    ]
    .join(","),
  );
  content.push('\n');
  for tweet in tweets {
    content.push_str(
      &vec![
        tweet.id.to_string(),
        tweet.published_time.to_string(),
        format!(
          "https://twitter.com/{}/status/{}",
          tweet.user_screen_name, tweet.id
        ),
        format!("\"{}\"", tweet.text.replace("\"", "\"\"")),
        tweet.user_name,
        format!("\"{}\"", tweet.urls.join(",")),
        tweet.reply_count.to_string(),
        tweet.retweet_count.to_string(),
        tweet.quote_count.to_string(),
      ]
      .join(","),
    );

    content.push('\n');
  }

  let headers = [
    (header::CONTENT_TYPE, "text/csv; charset=utf-8"),
    (
      header::CONTENT_DISPOSITION,
      &format!("attachment; filename=\"export_{}.csv\"", project.title),
    ),
  ];

  Ok((headers, content).into_response())
}
