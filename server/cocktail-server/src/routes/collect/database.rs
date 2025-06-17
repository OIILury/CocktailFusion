use crate::error::WebError;

// Constantes des URLs des APIs
pub const BLUESKY_API_URL: &str = "https://bsky.social/xrpc/app.bsky.feed.searchPosts";
pub const BLUESKY_AUTH_URL: &str = "https://bsky.social/xrpc/com.atproto.server.createSession";
pub const TWITTER_API_URL_RECENT: &str = "https://api.twitter.com/2/tweets/search/recent";
pub const TWITTER_API_URL_ALL: &str = "https://api.twitter.com/2/tweets/search/all";
pub const TWITTER_USER_URL: &str = "https://api.twitter.com/2/users/by/username";

/// Helper function to create tables for a schema
pub async fn create_collection_tables(pool: &sqlx::PgPool, schema_name: &str) -> Result<(), WebError> {
    // Créer le schéma s'il n'existe pas
    sqlx::query(&format!("CREATE SCHEMA IF NOT EXISTS {}", schema_name))
        .execute(pool)
        .await
        .map_err(|e| WebError::WTFError(format!("Failed to create schema: {}", e)))?;

    // Table User
    sqlx::query(&format!(
        r#"
        CREATE TABLE IF NOT EXISTS {}.user (
            id TEXT PRIMARY KEY,
            screen_name TEXT NOT NULL,
            name TEXT,
            created_at TEXT,
            verified BOOLEAN DEFAULT FALSE,
            protected BOOLEAN DEFAULT FALSE
        )
        "#,
        schema_name
    ))
    .execute(pool)
    .await
    .map_err(|e| WebError::WTFError(format!("Failed to create user table: {}", e)))?;

    // Table Place
    sqlx::query(&format!(
        r#"
        CREATE TABLE IF NOT EXISTS {}.place (
            id TEXT PRIMARY KEY,
            name TEXT,
            full_name TEXT,
            country_code TEXT,
            country TEXT,
            place_type TEXT,
            url TEXT,
            bounding_box TEXT,
            type_bounding_box TEXT
        )
        "#,
        schema_name
    ))
    .execute(pool)
    .await
    .map_err(|e| WebError::WTFError(format!("Failed to create place table: {}", e)))?;

    // Table Tweet (table centrale)
    sqlx::query(&format!(
        r#"
        CREATE TABLE IF NOT EXISTS {}.tweet (
            id TEXT PRIMARY KEY,
            created_at TEXT NOT NULL,
            published_time BIGINT NOT NULL,
            user_id TEXT NOT NULL REFERENCES {}.user(id),
            user_name TEXT NOT NULL,
            user_screen_name TEXT NOT NULL,
            text TEXT NOT NULL,
            source TEXT,
            language TEXT NOT NULL,
            coordinates_longitude TEXT,
            coordinates_latitude TEXT,
            possibly_sensitive BOOLEAN DEFAULT FALSE,
            retweet_count BIGINT NOT NULL DEFAULT 0,
            reply_count BIGINT NOT NULL DEFAULT 0,
            quote_count BIGINT NOT NULL DEFAULT 0
        )
        "#,
        schema_name, schema_name
    ))
    .execute(pool)
    .await
    .map_err(|e| WebError::WTFError(format!("Failed to create tweet table: {}", e)))?;

    // Table Reply
    sqlx::query(&format!(
        r#"
        CREATE TABLE IF NOT EXISTS {}.reply (
            tweet_id TEXT PRIMARY KEY REFERENCES {}.tweet(id),
            in_reply_to_tweet_id TEXT REFERENCES {}.tweet(id),
            in_reply_to_user_id TEXT REFERENCES {}.user(id),
            in_reply_to_screen_name TEXT
        )
        "#,
        schema_name, schema_name, schema_name, schema_name
    ))
    .execute(pool)
    .await
    .map_err(|e| WebError::WTFError(format!("Failed to create reply table: {}", e)))?;

    // Table Quote
    sqlx::query(&format!(
        r#"
        CREATE TABLE IF NOT EXISTS {}.quote (
            tweet_id TEXT PRIMARY KEY REFERENCES {}.tweet(id),
            quoted_tweet_id TEXT REFERENCES {}.tweet(id)
        )
        "#,
        schema_name, schema_name, schema_name
    ))
    .execute(pool)
    .await
    .map_err(|e| WebError::WTFError(format!("Failed to create quote table: {}", e)))?;

    // Table Retweet
    sqlx::query(&format!(
        r#"
        CREATE TABLE IF NOT EXISTS {}.retweet (
            tweet_id TEXT PRIMARY KEY REFERENCES {}.tweet(id),
            retweeted_tweet_id TEXT REFERENCES {}.tweet(id)
        )
        "#,
        schema_name, schema_name, schema_name
    ))
    .execute(pool)
    .await
    .map_err(|e| WebError::WTFError(format!("Failed to create retweet table: {}", e)))?;

    // Table Tweet_Hashtag
    sqlx::query(&format!(
        r#"
        CREATE TABLE IF NOT EXISTS {}.tweet_hashtag (
            tweet_id TEXT REFERENCES {}.tweet(id),
            hashtag TEXT NOT NULL,
            "order" INTEGER,
            start_indice INTEGER,
            end_indice INTEGER,
            PRIMARY KEY (tweet_id, hashtag)
        )
        "#,
        schema_name, schema_name
    ))
    .execute(pool)
    .await
    .map_err(|e| WebError::WTFError(format!("Failed to create tweet_hashtag table: {}", e)))?;

    // Table Tweet_Url
    sqlx::query(&format!(
        r#"
        CREATE TABLE IF NOT EXISTS {}.tweet_url (
            tweet_id TEXT REFERENCES {}.tweet(id),
            url TEXT NOT NULL,
            "order" INTEGER,
            start_indice INTEGER,
            end_indice INTEGER,
            PRIMARY KEY (tweet_id, url)
        )
        "#,
        schema_name, schema_name
    ))
    .execute(pool)
    .await
    .map_err(|e| WebError::WTFError(format!("Failed to create tweet_url table: {}", e)))?;

    // Table Tweet_Cashtag
    sqlx::query(&format!(
        r#"
        CREATE TABLE IF NOT EXISTS {}.tweet_cashtag (
            tweet_id TEXT REFERENCES {}.tweet(id),
            cashtag TEXT NOT NULL,
            "order" INTEGER,
            start_indice INTEGER,
            end_indice INTEGER,
            PRIMARY KEY (tweet_id, cashtag)
        )
        "#,
        schema_name, schema_name
    ))
    .execute(pool)
    .await
    .map_err(|e| WebError::WTFError(format!("Failed to create tweet_cashtag table: {}", e)))?;

    // Table Tweet_Emoji
    sqlx::query(&format!(
        r#"
        CREATE TABLE IF NOT EXISTS {}.tweet_emoji (
            tweet_id TEXT REFERENCES {}.tweet(id),
            emoji TEXT NOT NULL,
            "order" INTEGER,
            start_indice INTEGER,
            end_indice INTEGER,
            PRIMARY KEY (tweet_id, emoji)
        )
        "#,
        schema_name, schema_name
    ))
    .execute(pool)
    .await
    .map_err(|e| WebError::WTFError(format!("Failed to create tweet_emoji table: {}", e)))?;

    // Table Tweet_Media
    sqlx::query(&format!(
        r#"
        CREATE TABLE IF NOT EXISTS {}.tweet_media (
            tweet_id TEXT REFERENCES {}.tweet(id),
            media_url TEXT NOT NULL,
            type TEXT,
            "order" INTEGER,
            source_tweet_id TEXT,
            PRIMARY KEY (tweet_id, media_url)
        )
        "#,
        schema_name, schema_name
    ))
    .execute(pool)
    .await
    .map_err(|e| WebError::WTFError(format!("Failed to create tweet_media table: {}", e)))?;

    // Table Tweet_User_Mention
    sqlx::query(&format!(
        r#"
        CREATE TABLE IF NOT EXISTS {}.tweet_user_mention (
            tweet_id TEXT REFERENCES {}.tweet(id),
            user_id TEXT REFERENCES {}.user(id),
            "order" INTEGER,
            start_indice INTEGER,
            end_indice INTEGER,
            PRIMARY KEY (tweet_id, user_id)
        )
        "#,
        schema_name, schema_name, schema_name
    ))
    .execute(pool)
    .await
    .map_err(|e| WebError::WTFError(format!("Failed to create tweet_user_mention table: {}", e)))?;

    // Table Tweet_Place
    sqlx::query(&format!(
        r#"
        CREATE TABLE IF NOT EXISTS {}.tweet_place (
            tweet_id TEXT REFERENCES {}.tweet(id),
            place_id TEXT REFERENCES {}.place(id),
            PRIMARY KEY (tweet_id, place_id)
        )
        "#,
        schema_name, schema_name, schema_name
    ))
    .execute(pool)
    .await
    .map_err(|e| WebError::WTFError(format!("Failed to create tweet_place table: {}", e)))?;

    // Table Tweet_Keyword_User
    sqlx::query(&format!(
        r#"
        CREATE TABLE IF NOT EXISTS {}.tweet_keyword_user (
            tweet_id TEXT REFERENCES {}.tweet(id),
            user_id TEXT REFERENCES {}.user(id),
            PRIMARY KEY (tweet_id, user_id)
        )
        "#,
        schema_name, schema_name, schema_name
    ))
    .execute(pool)
    .await
    .map_err(|e| WebError::WTFError(format!("Failed to create tweet_keyword_user table: {}", e)))?;

    // Table Tweet_Keyword_Hashtag
    sqlx::query(&format!(
        r#"
        CREATE TABLE IF NOT EXISTS {}.tweet_keyword_hashtag (
            tweet_id TEXT REFERENCES {}.tweet(id),
            hashtag TEXT NOT NULL,
            PRIMARY KEY (tweet_id, hashtag)
        )
        "#,
        schema_name, schema_name
    ))
    .execute(pool)
    .await
    .map_err(|e| WebError::WTFError(format!("Failed to create tweet_keyword_hashtag table: {}", e)))?;

    // Table Withheld_In_Country
    sqlx::query(&format!(
        r#"
        CREATE TABLE IF NOT EXISTS {}.withheld_in_country (
            user_id TEXT REFERENCES {}.user(id),
            country TEXT NOT NULL,
            PRIMARY KEY (user_id, country)
        )
        "#,
        schema_name, schema_name
    ))
    .execute(pool)
    .await
    .map_err(|e| WebError::WTFError(format!("Failed to create withheld_in_country table: {}", e)))?;

    // Table Corpus (créée après Tweet pour respecter la référence)
    sqlx::query(&format!(
        r#"
        CREATE TABLE IF NOT EXISTS {}.corpus (
            tweet_id TEXT PRIMARY KEY REFERENCES {}.tweet(id),
            corpus TEXT NOT NULL
        )
        "#,
        schema_name, schema_name
    ))
    .execute(pool)
    .await
    .map_err(|e| WebError::WTFError(format!("Failed to create corpus table: {}", e)))?;

    Ok(())
} 