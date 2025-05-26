use axum::{
    extract::{Json, State},
    response::IntoResponse,
};
use chrono::{DateTime, Utc, Local};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use std::process::Command;

use crate::{
    error::WebError,
    models::auth::AuthenticatedUser,
    routes::paths::{ProjectCollect, StartCollection, DeleteCollection, UpdateCollection},
    AppState,
};

const BLUESKY_API_URL: &str = "https://bsky.social/xrpc/app.bsky.feed.searchPosts";
const BLUESKY_AUTH_URL: &str = "https://bsky.social/xrpc/com.atproto.server.createSession";

#[derive(Debug, Serialize, Deserialize)]
struct AuthResponse {
    accessJwt: String,
    refreshJwt: String,
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
    replyCount: i64,
    repostCount: i64,
    likeCount: i64,
    quoteCount: i64,
    indexedAt: String,
    #[serde(default)]
    embed: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PostRecord {
    #[serde(rename = "$type")]
    type_field: String,
    text: String,
    createdAt: String,
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
}

impl BlueskyCollector {
    async fn new(handle: &str, app_password: &str) -> Result<Self, WebError> {
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
        let auth = format!("Bearer {}", auth_response.accessJwt);
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

        Ok(Self { client, pool })
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
            r#"
            INSERT INTO cockt.tweet (
                id, created_at, published_time, user_id, user_name,
                user_screen_name, text, source, language,
                retweet_count, reply_count, quote_count
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            ON CONFLICT (id) DO NOTHING
            "#
        )
        .bind(tweet_id)
        .bind(created_at.format("%Y-%m-%d").to_string())
        .bind(created_at.timestamp())
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
        let created_at = DateTime::parse_from_rfc3339(&post.indexedAt)
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
            post.repostCount,
            post.replyCount,
            post.quoteCount
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
                r#"
                INSERT INTO cockt.reply (tweet_id, in_reply_to_tweet_id)
                VALUES ($1, $2)
                ON CONFLICT DO NOTHING
                "#
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
                                r#"
                                INSERT INTO cockt.quote (tweet_id, quoted_tweet_id)
                                VALUES ($1, $2)
                                ON CONFLICT DO NOTHING
                                "#
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
                                    r#"
                                    INSERT INTO cockt.tweet_hashtag (tweet_id, hashtag)
                                    VALUES ($1, $2)
                                    ON CONFLICT DO NOTHING
                                    "#
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
                                    r#"
                                    INSERT INTO cockt.tweet_url (tweet_id, url)
                                    VALUES ($1, $2)
                                    ON CONFLICT DO NOTHING
                                    "#
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

// Handler for starting a collection
pub async fn start_collection(
    path: StartCollection,
    State(_state): State<AppState>,
    _authuser: AuthenticatedUser,
    Json(req): Json<CollectionRequest>,
) -> Result<impl IntoResponse, WebError> {
    // Get Bluesky credentials from environment
    let handle = std::env::var("BLUESKY_HANDLE")
        .map_err(|_| WebError::WTFError("BLUESKY_HANDLE not set".to_string()))?;
    let app_password = std::env::var("BLUESKY_APP_PASSWORD")
        .map_err(|_| WebError::WTFError("BLUESKY_APP_PASSWORD not set".to_string()))?;
    
    // Create collector
    let collector = BlueskyCollector::new(&handle, &app_password).await?;
    
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
    
    // Save collection metadata to project
    sqlx::query(
        r#"
        INSERT INTO cockt.collection (
            project_id, name, keywords, networks, post_count
        ) VALUES ($1, $2, $3, $4, $5)
        "#
    )
    .bind(path.project_id)
    .bind(&req.name)
    .bind(&req.keywords)
    .bind(&req.networks)
    .bind(total_posts as i32)
    .execute(&collector.pool)
    .await
    .map_err(|e| WebError::WTFError(format!("Failed to save collection metadata: {}", e)))?;
    
    Ok(Json(CollectionResponse {
        success: true,
        message: format!("Successfully collected {} posts", total_posts),
        count: total_posts,
    }))
}

// Handler for deleting collected data
pub async fn delete_collection(
    path: DeleteCollection,
    State(_state): State<AppState>,
    _authuser: AuthenticatedUser,
) -> Result<impl IntoResponse, WebError> {
    // Get database connection
    let pool = sqlx::PgPool::connect(&std::env::var("PG_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://cocktailuser:cocktailuser@localhost:5432/cocktail_pg".to_string()))
        .await
        .map_err(|e| WebError::WTFError(format!("DB connection error: {}", e)))?;

    // Truncate all related tables
    sqlx::query(
        r#"
        TRUNCATE TABLE
            cockt.quote,
            cockt.reply,
            cockt.retweet,
            cockt.tweet,
            cockt.tweet_hashtag,
            cockt.tweet_url
        CASCADE;
        "#
    )
    .execute(&pool)
    .await
    .map_err(|e| WebError::WTFError(format!("Failed to truncate tables: {}", e)))?;

    // Delete collection metadata
    sqlx::query(
        r#"
        DELETE FROM cockt.collection 
        WHERE project_id = $1
        "#
    )
    .bind(path.project_id)
    .execute(&pool)
    .await
    .map_err(|e| WebError::WTFError(format!("Failed to delete collection metadata: {}", e)))?;

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
    let today = Local::now().format("%Y-%m-%d").to_string();
    let date_str = today.replace("-", "_");
    
    // Créer le dossier collecte s'il n'existe pas
    std::fs::create_dir_all("collecte").map_err(|e| WebError::WTFError(format!("Failed to create collecte directory: {}", e)))?;
    
    // Générer le JSON depuis la BDD
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!("echo \"{}\" | ./target/debug/tweets-from-sql-to-json | gzip -c > collecte/tweets_collecte_{}.json.gz", today, date_str))
        .output()
        .map_err(|e| WebError::WTFError(format!("Failed to generate JSON: {}", e)))?;
    
    if !output.status.success() {
        return Err(WebError::WTFError(format!("Failed to generate JSON: {}", String::from_utf8_lossy(&output.stderr))));
    }
    
    // Supprimer les anciens dossiers d'indexation
    let _ = std::fs::remove_dir_all("full-text-data");
    let _ = std::fs::remove_dir_all("tantivy-data");
    
    // Créer le nouvel index
    let output = Command::new("./target/debug/cocktail")
        .args(&["index", "create", "--directory-path", "tantivy-data"])
        .output()
        .map_err(|e| WebError::WTFError(format!("Failed to create index: {}", e)))?;
    
    if !output.status.success() {
        return Err(WebError::WTFError(format!("Failed to create index: {}", String::from_utf8_lossy(&output.stderr))));
    }
    
    // Ingérer les tweets dans l'index
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!("gunzip -c collecte/tweets_collecte_{}.json.gz | ./target/debug/cocktail index ingest --directory-path tantivy-data", date_str))
        .output()
        .map_err(|e| WebError::WTFError(format!("Failed to ingest tweets: {}", e)))?;
    
    if !output.status.success() {
        return Err(WebError::WTFError(format!("Failed to ingest tweets: {}", String::from_utf8_lossy(&output.stderr))));
    }
    
    // Supprimer l'ancienne base topk
    let _ = std::fs::remove_file("topk.db");
    
    // Générer les top hashtags et les insérer dans la base SQLite
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "./target/debug/topk --directory-path ./tantivy-data/ --query '*' | sqlite-utils insert --not-null key --not-null doc_count topk.db hashtag -"
        ))
        .output()
        .map_err(|e| WebError::WTFError(format!("Failed to generate and insert hashtags: {}", e)))?;
    
    if !output.status.success() {
        return Err(WebError::WTFError(format!("Failed to generate and insert hashtags: {}", String::from_utf8_lossy(&output.stderr))));
    }
    
    // Calculer et insérer les cooccurrences
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "./target/debug/topk_cooccurence --pg-database-url postgres://cocktailuser:cocktailuser@localhost:5432/cocktail_pg --schema cockt | sqlite-utils insert --not-null hashtag1 --not-null hashtag2 --not-null count topk.db hashtag_cooccurence -"
        ))
        .output()
        .map_err(|e| WebError::WTFError(format!("Failed to calculate and insert cooccurrences: {}", e)))?;
    
    if !output.status.success() {
        return Err(WebError::WTFError(format!("Failed to calculate and insert cooccurrences: {}", String::from_utf8_lossy(&output.stderr))));
    }
    
    Ok(Json(CollectionResponse {
        success: true,
        message: "Successfully updated all data".to_string(),
        count: 0,
    }))
} 