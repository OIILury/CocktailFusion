use std::path::PathBuf;

use sqlx::{
  migrate::{MigrateDatabase, MigrateError},
  Sqlite, SqlitePool,
};

#[tracing::instrument]
pub async fn create_database(database_path: &PathBuf) {
  let database_path = format!(
    "sqlite:{}",
    database_path
      .to_str()
      .expect("le chemin de la base de données n'est pas bon 🤔")
  );
  if let Ok(false) = Sqlite::database_exists(&database_path).await {
    Sqlite::create_database(&database_path)
      .await
      .expect("Erreur : impossible de créer la base de données.");
  }
}

#[tracing::instrument]
pub async fn migrate(pool: SqlitePool) -> Result<(), MigrateError> {
  sqlx::migrate!("./migrations").run(&pool).await
}
