use axum::{
    extract::{Json, State},
    response::IntoResponse,
};
use chrono::{DateTime, Utc, Local};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use serde::{Deserialize, Serialize};


use crate::{
    error::WebError,
    models::auth::AuthenticatedUser,
    routes::paths::{ProjectCollect, StartCollection, DeleteCollection, UpdateCollection},
    routes::automation::run_automation_pipeline,
    AppState,
};

const BLUESKY_API_URL: &str = "https://bsky.social/xrpc/app.bsky.feed.searchPosts";
const BLUESKY_AUTH_URL: &str = "https://bsky.social/xrpc/com.atproto.server.createSession";

#[derive(Debug, Serialize, Deserialize)]
struct AuthResponse {
    access_jwt: String,
    refresh_jwt: String,
    handle: String,
    did: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct SearchResponse {
    posts: Vec<Post>,
    #[serde(default)]
    cursor: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Post {
    uri: String,
    cid: String,
    author: Author,
    record: PostRecord,
    reply_count: i64,
    repost_count: i64,
    like_count: i64,
    quote_count: i64,
    indexed_at: String,
    #[serde(default)]
    embed: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PostRecord {
    #[serde(rename = "$type")]
    type_field: String,
    text: String,
    created_at: String,
    #[serde(default)]
    langs: Vec<String>,
    #[serde(default)]
    facets: Option<Vec<Facet>>,
    #[serde(default)]
    reply: Option<Reply>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Facet {
    features: Vec<FacetFeature>,
    index: FacetIndex,
}

#[derive(Debug, Serialize, Deserialize)]
struct FacetFeature {
    #[serde(rename = "$type")]
    type_field: String,
    #[serde(default)]
    tag: Option<String>,
    #[serde(default)]
    uri: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct FacetIndex {
    #[serde(rename = "byteStart")]
    byte_start: i32,
    #[serde(rename = "byteEnd")]
    byte_end: i32,
}

#[derive(Debug, Serialize, Deserialize)]
struct Author {
    did: String,
    handle: String,
    #[serde(rename = "displayName")]
    display_name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Reply {
    parent: ReplyData,
    root: ReplyData,
}

#[derive(Debug, Serialize, Deserialize)]
struct ReplyData {
    cid: String,
    uri: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CollectionRequest {
    name: String,
    keywords: Vec<String>,
    networks: Vec<String>,
    limit: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CollectionResponse {
    success: bool,
    message: String,
    count: usize,
}

struct BlueskyCollector {
    client: reqwest::Client,
    pool: sqlx::PgPool,
    schema_name: String,
}

impl BlueskyCollector {
    async fn new(handle: &str, app_password: &str, schema_name: String) -> Result<Self, WebError> {
        // Authenticate to get JWT token
        let auth_client = reqwest::Client::new();
        let auth_response = auth_client
            .post(BLUESKY_AUTH_URL)
            .json(&serde_json::json!({
                "identifier": handle,
                "password": app_password
            }))
            .send()
            .await
            .map_err(|e| WebError::WTFError(format!("Auth request error: {}", e)))?
            .json::<AuthResponse>()
            .await
            .map_err(|e| WebError::WTFError(format!("Auth response parse error: {}", e)))?;

        // Create client with JWT token
        let mut headers = HeaderMap::new();
        let auth = format!("Bearer {}", auth_response.access_jwt);
        headers.insert(
            AUTHORIZATION, 
            HeaderValue::from_str(&auth).map_err(|e| WebError::WTFError(format!("Header value error: {}", e)))?
        );
        
        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .map_err(|e| WebError::WTFError(format!("Client build error: {}", e)))?;

        // Use the existing PG connection from AppState
        let pool = sqlx::PgPool::connect(&std::env::var("PG_DATABASE_URL")
            .unwrap_or_else(|_| "postgres://cocktailuser:cocktailuser@localhost:5432/cocktail_pg".to_string()))
            .await
            .map_err(|e| WebError::WTFError(format!("DB connection error: {}", e)))?;

        Ok(Self { client, pool, schema_name })
    }

    async fn search_posts(&self, keyword: &str, limit: usize) -> Result<Vec<Post>, WebError> {
        let mut posts = Vec::new();
        let mut cursor: Option<String> = None;
        
        while posts.len() < limit {
            let batch_limit = (limit - posts.len()).min(10);
            let mut params = vec![
                ("q", keyword.to_string()),
                ("limit", batch_limit.to_string()),
            ];
            
            if let Some(cursor_str) = cursor.as_ref() {
                params.push(("cursor", cursor_str.to_string()));
            }
            
            let response = self.client
                .get(BLUESKY_API_URL)
                .query(&params)
                .send()
                .await
                .map_err(|e| WebError::WTFError(format!("API request error: {}", e)))?;
            
            if !response.status().is_success() {
                return Err(WebError::WTFError(format!("API error: {}", response.status())));
            }
            
            let search_response: SearchResponse = response
                .json()
                .await
                .map_err(|e| WebError::WTFError(format!("API response parse error: {}", e)))?;
            
            if search_response.posts.is_empty() {
                break;
            }
            
            posts.extend(search_response.posts);
            cursor = search_response.cursor;
            
            if cursor.is_none() {
                break;
            }
        }
        
        Ok(posts.into_iter().take(limit).collect())
    }

    // Helper to get tweet ID from URI
    fn extract_tweet_id(uri: &str) -> String {
        uri.split('/').last().unwrap_or_default().to_string()
    }

    // Helper to insert a basic tweet
    async fn insert_basic_tweet(&self, tweet_id: &str, created_at: DateTime<Utc>, author: &Author, 
                               text: &str, source: &str, lang: &str, repost_count: i64, 
                               reply_count: i64, quote_count: i64) -> Result<(), WebError> {
        sqlx::query(
            &format!(r#"
            INSERT INTO {}.tweet (
                id, created_at, published_time, user_id, user_name,
                user_screen_name, text, source, language,
                retweet_count, reply_count, quote_count
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            ON CONFLICT (id) DO NOTHING
            "#, self.schema_name)
        )
        .bind(tweet_id)
        .bind(created_at.format("%Y-%m-%d").to_string())
        .bind(created_at.timestamp_millis())
        .bind(author.did.clone())
        .bind(author.display_name.as_deref().unwrap_or_default())
        .bind(author.handle.clone())
        .bind(text)
        .bind(source)
        .bind(lang)
        .bind(repost_count)
        .bind(reply_count)
        .bind(quote_count)
        .execute(&self.pool)
        .await
        .map_err(|e| WebError::WTFError(format!("DB insert error: {}", e)))?;
        
        Ok(())
    }

    async fn save_post_to_db(&self, post: &Post) -> Result<(), WebError> {
        let created_at = DateTime::parse_from_rfc3339(&post.indexed_at)
            .map_err(|e| WebError::WTFError(format!("Date parse error: {}", e)))?
            .with_timezone(&Utc);
            
        let default_lang = "fr".to_string();
        let lang = post.record.langs.first().unwrap_or(&default_lang);
        let tweet_id = Self::extract_tweet_id(&post.uri);
        
        // Insert main tweet
        self.insert_basic_tweet(
            &tweet_id, 
            created_at, 
            &post.author,
            &post.record.text,
            "Bluesky",
            lang,
            post.repost_count,
            post.reply_count,
            post.quote_count
        ).await?;
        
        // Handle replies
        if let Some(reply) = &post.record.reply {
            let parent_tweet_id = Self::extract_tweet_id(&reply.parent.uri);
            
            // Insert minimal parent tweet if it doesn't exist
            self.insert_basic_tweet(
                &parent_tweet_id,
                created_at,
                &post.author,
                "Parent tweet",
                "Bluesky",
                "fr",
                0, 0, 0
            ).await?;
            
            // Insert reply reference
            sqlx::query(
                &format!(r#"
                INSERT INTO {}.reply (tweet_id, in_reply_to_tweet_id)
                VALUES ($1, $2)
                ON CONFLICT DO NOTHING
                "#, self.schema_name)
            )
            .bind(&tweet_id)
            .bind(&parent_tweet_id)
            .execute(&self.pool)
            .await
            .map_err(|e| WebError::WTFError(format!("DB insert reply error: {}", e)))?;
        }
        
        // Handle embeds (potential quotes)
        if let Some(embed_value) = &post.embed {
            // Check if it's a record embed (quote)
            if let Some(record) = embed_value.get("record") {
                if let Some(record_data) = record.get("record") {
                    if let Some(uri) = record_data.get("uri") {
                        if let Some(uri_str) = uri.as_str() {
                            let quoted_tweet_id = Self::extract_tweet_id(uri_str);
                            
                            // Insert minimal quoted tweet if it doesn't exist
                            self.insert_basic_tweet(
                                &quoted_tweet_id,
                                created_at,
                                &post.author,
                                "Quoted tweet",
                                "Bluesky",
                                "fr",
                                0, 0, 0
                            ).await?;
                            
                            // Insert quote reference
                            sqlx::query(
                                &format!(r#"
                                INSERT INTO {}.quote (tweet_id, quoted_tweet_id)
                                VALUES ($1, $2)
                                ON CONFLICT DO NOTHING
                                "#, self.schema_name)
                            )
                            .bind(&tweet_id)
                            .bind(&quoted_tweet_id)
                            .execute(&self.pool)
                            .await
                            .map_err(|e| WebError::WTFError(format!("DB insert quote error: {}", e)))?;
                        }
                    }
                }
            }
        }
        
        // Insert hashtags and URLs if facets exist
        if let Some(facets) = &post.record.facets {
            for facet in facets {
                for feature in &facet.features {
                    match feature.type_field.as_str() {
                        "app.bsky.richtext.facet#tag" => {
                            if let Some(tag) = &feature.tag {
                                sqlx::query(
                                    &format!(r#"
                                    INSERT INTO {}.tweet_hashtag (tweet_id, hashtag)
                                    VALUES ($1, $2)
                                    ON CONFLICT DO NOTHING
                                    "#, self.schema_name)
                                )
                                .bind(&tweet_id)
                                .bind(tag)
                                .execute(&self.pool)
                                .await
                                .map_err(|e| WebError::WTFError(format!("DB insert hashtag error: {}", e)))?;
                            }
                        },
                        "app.bsky.richtext.facet#link" => {
                            if let Some(uri) = &feature.uri {
                                sqlx::query(
                                    &format!(r#"
                                    INSERT INTO {}.tweet_url (tweet_id, url)
                                    VALUES ($1, $2)
                                    ON CONFLICT DO NOTHING
                                    "#, self.schema_name)
                                )
                                .bind(&tweet_id)
                                .bind(uri)
                                .execute(&self.pool)
                                .await
                                .map_err(|e| WebError::WTFError(format!("DB insert URL error: {}", e)))?;
                            }
                        },
                        _ => {}
                    }
                }
            }
        }
        
        Ok(())
    }
}

// Handler for the collect page
pub async fn collect(
    State(_state): State<AppState>,
    _authuser: AuthenticatedUser,
    paths: ProjectCollect,
) -> impl IntoResponse {
    // Render the collect page
    let mut hbs_registry = handlebars::Handlebars::new();
    hbs_registry.register_template_string("collect", include_str!("../../templates/collect.html")).unwrap();
    
    let data = serde_json::json!({
        "project_id": paths.project_id,
        "is_analyzed": false
    });
    
    hbs_registry.render("collect", &data).unwrap()
}

// Helper function to create tables for a schema
async fn create_collection_tables(pool: &sqlx::PgPool, schema_name: &str) -> Result<(), WebError> {
    // Create schema if it doesn't exist
    sqlx::query(&format!("CREATE SCHEMA IF NOT EXISTS {}", schema_name))
        .execute(pool)
        .await
        .map_err(|e| WebError::WTFError(format!("Failed to create schema: {}", e)))?;

    // Table tweet
    sqlx::query(&format!(
        r#"
        CREATE TABLE IF NOT EXISTS {}.tweet (
            id TEXT PRIMARY KEY,
            created_at TEXT NOT NULL,
            published_time BIGINT NOT NULL,
            user_id TEXT NOT NULL,
            user_name TEXT NOT NULL,
            user_screen_name TEXT NOT NULL,
            text TEXT NOT NULL,
            source TEXT,
            language TEXT NOT NULL,
            coordinates_longitude TEXT,
            coordinates_latitude TEXT,
            possibly_sensitive BOOLEAN,
            retweet_count BIGINT NOT NULL DEFAULT 0,
            reply_count BIGINT NOT NULL DEFAULT 0,
            quote_count BIGINT NOT NULL DEFAULT 0
        )
        "#,
        schema_name
    ))
    .execute(pool)
    .await
    .map_err(|e| WebError::WTFError(format!("Failed to create tweet table: {}", e)))?;

    // Table tweet_hashtag
    sqlx::query(&format!(
        "CREATE TABLE IF NOT EXISTS {}.tweet_hashtag (tweet_id TEXT REFERENCES {}.tweet(id), hashtag TEXT)",
        schema_name, schema_name
    ))
    .execute(pool)
    .await
    .map_err(|e| WebError::WTFError(format!("Failed to create tweet_hashtag table: {}", e)))?;

    // Table tweet_url
    sqlx::query(&format!(
        "CREATE TABLE IF NOT EXISTS {}.tweet_url (tweet_id TEXT REFERENCES {}.tweet(id), url TEXT)",
        schema_name, schema_name
    ))
    .execute(pool)
    .await
    .map_err(|e| WebError::WTFError(format!("Failed to create tweet_url table: {}", e)))?;

    // Table retweet
    sqlx::query(&format!(
        "CREATE TABLE IF NOT EXISTS {}.retweet (retweeted_tweet_id TEXT REFERENCES {}.tweet(id))",
        schema_name, schema_name
    ))
    .execute(pool)
    .await
    .map_err(|e| WebError::WTFError(format!("Failed to create retweet table: {}", e)))?;

    // Table reply
    sqlx::query(&format!(
        "CREATE TABLE IF NOT EXISTS {}.reply (tweet_id TEXT REFERENCES {}.tweet(id), in_reply_to_tweet_id TEXT REFERENCES {}.tweet(id))",
        schema_name, schema_name, schema_name
    ))
    .execute(pool)
    .await
    .map_err(|e| WebError::WTFError(format!("Failed to create reply table: {}", e)))?;

    // Table quote
    sqlx::query(&format!(
        "CREATE TABLE IF NOT EXISTS {}.quote (tweet_id TEXT REFERENCES {}.tweet(id), quoted_tweet_id TEXT REFERENCES {}.tweet(id))",
        schema_name, schema_name, schema_name
    ))
    .execute(pool)
    .await
    .map_err(|e| WebError::WTFError(format!("Failed to create quote table: {}", e)))?;

    Ok(())
}

// Handler for starting a collection
pub async fn start_collection(
    path: StartCollection,
    State(_state): State<AppState>,
    _authuser: AuthenticatedUser,
    Json(req): Json<CollectionRequest>,
) -> Result<impl IntoResponse, WebError> {
    // Create schema name based on current date
    let schema_name = format!("collect_{}", Local::now().format("%Y%m%d"));
    
    // Get database connection
    let pool = sqlx::PgPool::connect(&std::env::var("PG_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://cocktailuser:cocktailuser@localhost:5432/cocktail_pg".to_string()))
        .await
        .map_err(|e| WebError::WTFError(format!("DB connection error: {}", e)))?;

    // Create tables for the schema
    create_collection_tables(&pool, &schema_name).await?;

    // Get Bluesky credentials from environment
    let handle = std::env::var("BLUESKY_HANDLE")
        .map_err(|_| WebError::WTFError("BLUESKY_HANDLE not set".to_string()))?;
    let app_password = std::env::var("BLUESKY_APP_PASSWORD")
        .map_err(|_| WebError::WTFError("BLUESKY_APP_PASSWORD not set".to_string()))?;
    
    // Create collector with schema name
    let collector = BlueskyCollector::new(&handle, &app_password, schema_name.clone()).await?;
    
    let mut total_posts = 0;
    
    // Process each keyword
    for keyword in &req.keywords {
        // Default limit to 10 if not specified
        let limit = req.limit.unwrap_or(10);
        
        // Search for posts
        let posts = collector.search_posts(keyword, limit).await?;
        
        // Save posts to database
        for post in &posts {
            if let Err(e) = collector.save_post_to_db(post).await {
                tracing::warn!("Error saving post: {}", e);
            } else {
                total_posts += 1;
            }
        }
    }
    
    // Run automation pipeline if we have collected data
    if total_posts > 0 {
        tracing::info!("Starting automation pipeline for schema {}", schema_name);
        if let Err(e) = run_automation_pipeline(&schema_name, Some(path.project_id.to_string())).await {
            tracing::error!("Error during automation pipeline: {}", e);
            return Ok(Json(CollectionResponse {
                success: false,
                message: format!("Collection successful but automation failed: {}", e),
                count: total_posts,
            }));
        }
    }
    
    Ok(Json(CollectionResponse {
        success: true,
        message: format!("Successfully collected {} posts", total_posts),
        count: total_posts,
    }))
}

// Handler for deleting collected data
pub async fn delete_collection(
    _path: DeleteCollection,
    State(_state): State<AppState>,
    _authuser: AuthenticatedUser,
) -> Result<impl IntoResponse, WebError> {
    // Get database connection
    let pool = sqlx::PgPool::connect(&std::env::var("PG_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://cocktailuser:cocktailuser@localhost:5432/cocktail_pg".to_string()))
        .await
        .map_err(|e| WebError::WTFError(format!("DB connection error: {}", e)))?;

    // Get all collection schemas (collect_YYYYMMDD pattern)
    let schema_query = "SELECT schema_name FROM information_schema.schemata WHERE schema_name LIKE 'collect_%'";
    let collections = sqlx::query_scalar::<_, String>(schema_query)
        .fetch_all(&pool)
        .await
        .map_err(|e| WebError::WTFError(format!("Failed to get collection schemas: {}", e)))?;

    // Drop each collection schema
    for schema_name in &collections {
        sqlx::query(&format!("DROP SCHEMA IF EXISTS {} CASCADE", schema_name))
            .execute(&pool)
            .await
            .map_err(|e| WebError::WTFError(format!("Failed to drop schema {}: {}", schema_name, e)))?;
    }

    Ok(Json(CollectionResponse {
        success: true,
        message: "Successfully deleted all collected data".to_string(),
        count: 0,
    }))
}

pub async fn update_collection(
    path: UpdateCollection,
    State(_state): State<AppState>,
    _authuser: AuthenticatedUser,
) -> Result<impl IntoResponse, WebError> {
    // Get the most recent collection schema (collect_YYYYMMDD pattern)
    let pool = sqlx::PgPool::connect(&std::env::var("PG_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://cocktailuser:cocktailuser@localhost:5432/cocktail_pg".to_string()))
        .await
        .map_err(|e| WebError::WTFError(format!("DB connection error: {}", e)))?;

    let schema_query = "SELECT schema_name FROM information_schema.schemata WHERE schema_name LIKE 'collect_%' ORDER BY schema_name DESC LIMIT 1";
    let schema_name = sqlx::query_scalar::<_, String>(schema_query)
        .fetch_optional(&pool)
        .await
        .map_err(|e| WebError::WTFError(format!("Failed to get collection schema: {}", e)))?
        .unwrap_or_else(|| format!("collect_{}", Local::now().format("%Y%m%d")));

    tracing::info!("Starting automation pipeline for schema {} and project {}", schema_name, path.project_id);
    
    // Use the automation pipeline to update data
    if let Err(e) = run_automation_pipeline(&schema_name, Some(path.project_id.to_string())).await {
        tracing::error!("Error during automation pipeline: {}", e);
        return Err(WebError::WTFError(format!("Failed to update data: {}", e)));
    }
    
    Ok(Json(CollectionResponse {
        success: true,
        message: "Successfully updated all data using automation pipeline".to_string(),
        count: 0,
    }))
} 