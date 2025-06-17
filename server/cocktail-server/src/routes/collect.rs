use axum::{
    extract::{Json, State},
    response::IntoResponse,
    http::HeaderMap,
};
use chrono::{DateTime, Utc, Local};
use reqwest::header::{HeaderMap as ReqwestHeaderMap, HeaderValue, AUTHORIZATION};
use serde::{Deserialize, Serialize};
use regex::Regex;


use crate::{
    error::WebError,
    models::{auth::AuthenticatedUser, templates::{self, HtmlTemplate}},
    routes::paths::{
        ProjectCollect, StartCollection, DeleteCollection, UpdateCollection,
        ProjectDateRange, ProjectHashtags, ProjectRequest, ProjectImport,
        PopupDeleteProject, PopupRenameProject, PopupDuplicateProject,
        DownloadProject, PopupAnalysisPreview, ProjectAnalysis,
        ProjectResults, ProjectTweetsGraph, ProjectAuthors,
        ProjectResultHashtags, Communities
    },
    routes::automation::run_automation_pipeline,
    get_logout_url,
    AppState,
};

const BLUESKY_API_URL: &str = "https://bsky.social/xrpc/app.bsky.feed.searchPosts";
const BLUESKY_AUTH_URL: &str = "https://bsky.social/xrpc/com.atproto.server.createSession";
const TWITTER_API_URL_RECENT: &str = "https://api.twitter.com/2/tweets/search/recent";
const TWITTER_API_URL_ALL: &str = "https://api.twitter.com/2/tweets/search/all";
const TWITTER_USER_URL: &str = "https://api.twitter.com/2/users/by/username";

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

// Twitter API structures
#[derive(Debug, Serialize, Deserialize)]
struct TwitterSearchResponse {
    data: Option<Vec<TwitterTweet>>,
    includes: Option<TwitterIncludes>,
    #[serde(default)]
    meta: Option<TwitterMeta>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TwitterTweet {
    id: String,
    text: String,
    created_at: String,
    author_id: String,
    #[serde(default)]
    context_annotations: Option<Vec<TwitterContextAnnotation>>,
    #[serde(default)]
    entities: Option<TwitterEntities>,
    #[serde(default)]
    geo: Option<TwitterGeo>,
    #[serde(default)]
    in_reply_to_user_id: Option<String>,
    #[serde(default)]
    lang: Option<String>,
    #[serde(default)]
    possibly_sensitive: Option<bool>,
    public_metrics: Option<TwitterPublicMetrics>,
    #[serde(default)]
    referenced_tweets: Option<Vec<TwitterReferencedTweet>>,
    #[serde(default)]
    reply_settings: Option<String>,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    withheld: Option<TwitterWithheld>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TwitterIncludes {
    #[serde(default)]
    users: Option<Vec<TwitterUser>>,
    #[serde(default)]
    tweets: Option<Vec<TwitterTweet>>,
    #[serde(default)]
    places: Option<Vec<TwitterPlace>>,
    #[serde(default)]
    media: Option<Vec<TwitterMedia>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TwitterUser {
    id: String,
    username: String,
    name: String,
    created_at: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    location: Option<String>,
    #[serde(default)]
    pinned_tweet_id: Option<String>,
    #[serde(default)]
    profile_image_url: Option<String>,
    #[serde(default)]
    protected: Option<bool>,
    public_metrics: Option<TwitterUserPublicMetrics>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    verified: Option<bool>,
    #[serde(default)]
    withheld: Option<TwitterWithheld>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TwitterMeta {
    newest_id: Option<String>,
    oldest_id: Option<String>,
    result_count: i32,
    #[serde(default)]
    next_token: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TwitterContextAnnotation {
    domain: TwitterDomain,
    entity: TwitterEntity,
}

#[derive(Debug, Serialize, Deserialize)]
struct TwitterDomain {
    id: String,
    name: String,
    description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TwitterEntity {
    id: String,
    name: String,
    description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TwitterEntities {
    #[serde(default)]
    annotations: Option<Vec<TwitterAnnotation>>,
    #[serde(default)]
    cashtags: Option<Vec<TwitterCashtag>>,
    #[serde(default)]
    hashtags: Option<Vec<TwitterHashtag>>,
    #[serde(default)]
    mentions: Option<Vec<TwitterMention>>,
    #[serde(default)]
    urls: Option<Vec<TwitterUrl>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TwitterAnnotation {
    start: i32,
    end: i32,
    probability: f64,
    #[serde(rename = "type")]
    annotation_type: String,
    normalized_text: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TwitterCashtag {
    start: i32,
    end: i32,
    tag: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct TwitterHashtag {
    start: i32,
    end: i32,
    tag: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct TwitterMention {
    start: i32,
    end: i32,
    username: String,
    id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TwitterUrl {
    start: i32,
    end: i32,
    url: String,
    expanded_url: Option<String>,
    display_url: Option<String>,
    media_key: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TwitterGeo {
    coordinates: Option<TwitterCoordinates>,
    place_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TwitterCoordinates {
    #[serde(rename = "type")]
    coord_type: String,
    coordinates: Vec<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TwitterPublicMetrics {
    retweet_count: i64,
    reply_count: i64,
    like_count: i64,
    quote_count: i64,
    #[serde(default)]
    bookmark_count: Option<i64>,
    #[serde(default)]
    impression_count: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TwitterUserPublicMetrics {
    followers_count: i64,
    following_count: i64,
    tweet_count: i64,
    listed_count: i64,
}

#[derive(Debug, Serialize, Deserialize)]
struct TwitterReferencedTweet {
    #[serde(rename = "type")]
    ref_type: String, // "retweeted", "quoted", "replied_to"
    id: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct TwitterPlace {
    id: String,
    full_name: String,
    name: String,
    country: Option<String>,
    country_code: Option<String>,
    geo: Option<TwitterPlaceGeo>,
    place_type: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TwitterPlaceGeo {
    #[serde(rename = "type")]
    geo_type: String,
    bbox: Option<Vec<f64>>,
    properties: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TwitterMedia {
    media_key: String,
    #[serde(rename = "type")]
    media_type: String,
    url: Option<String>,
    duration_ms: Option<i64>,
    height: Option<i64>,
    preview_image_url: Option<String>,
    public_metrics: Option<TwitterMediaMetrics>,
    width: Option<i64>,
    alt_text: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TwitterMediaMetrics {
    view_count: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TwitterWithheld {
    copyright: Option<bool>,
    country_codes: Option<Vec<String>>,
    scope: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CollectionRequest {
    name: String,
    keywords: Vec<String>,
    networks: Vec<String>,
    limit: Option<usize>,
    start_date: Option<String>, // Format ISO 8601
    end_date: Option<String>,   // Format ISO 8601
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
        let mut headers = ReqwestHeaderMap::new();
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

        Ok(Self { client, pool, schema_name })
    }

    async fn search_posts(&self, keyword: &str, limit: usize, start_date: Option<&str>, end_date: Option<&str>) -> Result<Vec<Post>, WebError> {
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
            
            // Filtrer les posts par date si nécessaire (côté client pour Bluesky)
            let original_count = search_response.posts.len();
            let mut filtered_posts = search_response.posts;
            if start_date.is_some() || end_date.is_some() {
                filtered_posts = filtered_posts.into_iter().filter(|post| {
                    if let Ok(post_date) = DateTime::parse_from_rfc3339(&post.indexedAt) {
                        let mut valid = true;
                        
                        if let Some(start) = start_date {
                            if let Ok(start_dt) = DateTime::parse_from_rfc3339(start) {
                                valid = valid && post_date >= start_dt;
                            }
                        }
                        
                        if let Some(end) = end_date {
                            if let Ok(end_dt) = DateTime::parse_from_rfc3339(end) {
                                valid = valid && post_date <= end_dt;
                            }
                        }
                        
                        valid
                    } else {
                        true // Garder les posts avec des dates invalides
                    }
                }).collect();
                
                tracing::info!("Filtered Bluesky posts from {} to {} posts", original_count, filtered_posts.len());
            }
            
            posts.extend(filtered_posts);
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

    // Helper pour insérer un utilisateur
    async fn insert_user(&self, author: &Author, created_at: DateTime<Utc>) -> Result<(), WebError> {
        sqlx::query(
            &format!(r#"
            INSERT INTO {}.user (
                id, screen_name, name, created_at, verified, protected
            ) VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (id) DO NOTHING
            "#, self.schema_name)
        )
        .bind(&author.did)
        .bind(&author.handle)
        .bind(author.display_name.as_deref().unwrap_or_default())
        .bind(created_at.format("%Y-%m-%d").to_string())
        .bind(false) // verified - par défaut false pour Bluesky
        .bind(false) // protected - par défaut false pour Bluesky
        .execute(&self.pool)
        .await
        .map_err(|e| WebError::WTFError(format!("DB insert user error: {}", e)))?;
        
        Ok(())
    }

    // Helper pour insérer un tweet de base
    async fn insert_basic_tweet(&self, tweet_id: &str, created_at: DateTime<Utc>, author: &Author, 
                               text: &str, source: &str, lang: &str, retweet_count: i64, 
                               reply_count: i64, quote_count: i64) -> Result<(), WebError> {
        // D'abord insérer l'utilisateur
        self.insert_user(author, created_at).await?;
        
        // Puis insérer le tweet
        sqlx::query(
            &format!(r#"
            INSERT INTO {}.tweet (
                id, created_at, published_time, user_id, user_name,
                user_screen_name, text, source, language,
                coordinates_longitude, coordinates_latitude, possibly_sensitive,
                retweet_count, reply_count, quote_count
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
            ON CONFLICT (id) DO NOTHING
            "#, self.schema_name)
        )
        .bind(tweet_id)
        .bind(created_at.format("%Y-%m-%d").to_string())
        .bind(created_at.timestamp_millis())
        .bind(&author.did)
        .bind(author.display_name.as_deref().unwrap_or_default())
        .bind(&author.handle)
        .bind(text)
        .bind(source)
        .bind(lang)
        .bind::<Option<String>>(None) // coordinates_longitude
        .bind::<Option<String>>(None) // coordinates_latitude
        .bind(false) // possibly_sensitive
        .bind(retweet_count)
        .bind(reply_count)
        .bind(quote_count)
        .execute(&self.pool)
        .await
        .map_err(|e| WebError::WTFError(format!("DB insert tweet error: {}", e)))?;
        
        Ok(())
    }

    // Helper pour insérer des hashtags avec indices
    async fn insert_hashtags(&self, tweet_id: &str, facets: &[Facet]) -> Result<(), WebError> {
        for (order, facet) in facets.iter().enumerate() {
            for feature in &facet.features {
                if feature.type_field == "app.bsky.richtext.facet#tag" {
                    if let Some(tag) = &feature.tag {
                        sqlx::query(
                            &format!(r#"
                            INSERT INTO {}.tweet_hashtag (tweet_id, hashtag, "order", start_indice, end_indice)
                            VALUES ($1, $2, $3, $4, $5)
                            ON CONFLICT (tweet_id, hashtag) DO NOTHING
                            "#, self.schema_name)
                        )
                        .bind(tweet_id)
                        .bind(tag)
                        .bind(order as i32)
                        .bind(facet.index.byte_start)
                        .bind(facet.index.byte_end)
                        .execute(&self.pool)
                        .await
                        .map_err(|e| WebError::WTFError(format!("DB insert hashtag error: {}", e)))?;
                    }
                }
            }
        }
        Ok(())
    }

    // Helper pour insérer des URLs avec indices
    async fn insert_urls(&self, tweet_id: &str, facets: &[Facet]) -> Result<(), WebError> {
        for (order, facet) in facets.iter().enumerate() {
            for feature in &facet.features {
                if feature.type_field == "app.bsky.richtext.facet#link" {
                    if let Some(uri) = &feature.uri {
                        sqlx::query(
                            &format!(r#"
                            INSERT INTO {}.tweet_url (tweet_id, url, "order", start_indice, end_indice)
                            VALUES ($1, $2, $3, $4, $5)
                            ON CONFLICT (tweet_id, url) DO NOTHING
                            "#, self.schema_name)
                        )
                        .bind(tweet_id)
                        .bind(uri)
                        .bind(order as i32)
                        .bind(facet.index.byte_start)
                        .bind(facet.index.byte_end)
                        .execute(&self.pool)
                        .await
                        .map_err(|e| WebError::WTFError(format!("DB insert URL error: {}", e)))?;
                    }
                }
            }
        }
        Ok(())
    }

    // Helper pour extraire et insérer des emojis du texte
    async fn insert_emojis(&self, tweet_id: &str, text: &str) -> Result<(), WebError> {
        // Regex simple pour capturer les emojis communs
        let emoji_regex = Regex::new(r"[\u{1F600}-\u{1F64F}]|[\u{1F300}-\u{1F5FF}]|[\u{1F680}-\u{1F6FF}]|[\u{1F1E0}-\u{1F1FF}]|[\u{2600}-\u{26FF}]|[\u{2700}-\u{27BF}]").unwrap();
        let mut order = 0;
        
        for emoji_match in emoji_regex.find_iter(text) {
            sqlx::query(
                &format!(r#"
                INSERT INTO {}.tweet_emoji (tweet_id, emoji, "order", start_indice, end_indice)
                VALUES ($1, $2, $3, $4, $5)
                ON CONFLICT (tweet_id, emoji) DO NOTHING
                "#, self.schema_name)
            )
            .bind(tweet_id)
            .bind(emoji_match.as_str())
            .bind(order)
            .bind(emoji_match.start() as i32)
            .bind(emoji_match.end() as i32)
            .execute(&self.pool)
            .await
            .map_err(|e| WebError::WTFError(format!("DB insert emoji error: {}", e)))?;
            
            order += 1;
        }
        Ok(())
    }

    // Helper pour extraire et insérer des cashtags du texte
    async fn insert_cashtags(&self, tweet_id: &str, text: &str) -> Result<(), WebError> {
        let cashtag_regex = Regex::new(r"\$[A-Za-z][A-Za-z0-9]*").unwrap();
        let mut order = 0;
        
        for cashtag_match in cashtag_regex.find_iter(text) {
            sqlx::query(
                &format!(r#"
                INSERT INTO {}.tweet_cashtag (tweet_id, cashtag, "order", start_indice, end_indice)
                VALUES ($1, $2, $3, $4, $5)
                ON CONFLICT (tweet_id, cashtag) DO NOTHING
                "#, self.schema_name)
            )
            .bind(tweet_id)
            .bind(cashtag_match.as_str())
            .bind(order)
            .bind(cashtag_match.start() as i32)
            .bind(cashtag_match.end() as i32)
            .execute(&self.pool)
            .await
            .map_err(|e| WebError::WTFError(format!("DB insert cashtag error: {}", e)))?;
            
            order += 1;
        }
        Ok(())
    }

    // Helper pour insérer des médias depuis les embeds (seulement URL et type)
    async fn insert_media(&self, tweet_id: &str, embed: &serde_json::Value) -> Result<(), WebError> {
        let mut order = 0;
        
        // Traiter les images
        if let Some(images) = embed.get("images") {
            if let Some(images_array) = images.as_array() {
                for image in images_array {
                    if let Some(fullsize) = image.get("fullsize") {
                        if let Some(media_url) = fullsize.as_str() {
                            sqlx::query(
                                &format!(r#"
                                INSERT INTO {}.tweet_media (tweet_id, media_url, type, "order")
                                VALUES ($1, $2, $3, $4)
                                ON CONFLICT (tweet_id, media_url) DO NOTHING
                                "#, self.schema_name)
                            )
                            .bind(tweet_id)
                            .bind(media_url)
                            .bind("image")
                            .bind(order)
                            .execute(&self.pool)
                            .await
                            .map_err(|e| WebError::WTFError(format!("DB insert media error: {}", e)))?;
                            
                            order += 1;
                        }
                    }
                }
            }
        }
        
        // Traiter les vidéos
        if let Some(video) = embed.get("video") {
            if let Some(playlist) = video.get("playlist") {
                if let Some(media_url) = playlist.as_str() {
                    sqlx::query(
                        &format!(r#"
                        INSERT INTO {}.tweet_media (tweet_id, media_url, type, "order")
                        VALUES ($1, $2, $3, $4)
                        ON CONFLICT (tweet_id, media_url) DO NOTHING
                        "#, self.schema_name)
                    )
                    .bind(tweet_id)
                    .bind(media_url)
                    .bind("video")
                    .bind(order)
                    .execute(&self.pool)
                    .await
                    .map_err(|e| WebError::WTFError(format!("DB insert media error: {}", e)))?;
                    
                    order += 1;
                }
            }
        }
        
        // Traiter les liens externes (pour les médias externes)
        if let Some(external) = embed.get("external") {
            if let Some(uri) = external.get("uri") {
                if let Some(media_url) = uri.as_str() {
                    // Déterminer le type basé sur l'extension de l'URL
                    let media_type = if media_url.contains("youtube.com") || media_url.contains("youtu.be") {
                        "video"
                    } else if media_url.contains("instagram.com") || media_url.contains("tiktok.com") {
                        "social_media"
                    } else {
                        "link"
                    };
                    
                    sqlx::query(
                        &format!(r#"
                        INSERT INTO {}.tweet_media (tweet_id, media_url, type, "order")
                        VALUES ($1, $2, $3, $4)
                        ON CONFLICT (tweet_id, media_url) DO NOTHING
                        "#, self.schema_name)
                    )
                    .bind(tweet_id)
                    .bind(media_url)
                    .bind(media_type)
                    .bind(order)
                    .execute(&self.pool)
                    .await
                    .map_err(|e| WebError::WTFError(format!("DB insert media error: {}", e)))?;
                }
            }
        }
        
        Ok(())
    }

    async fn save_post_to_db(&self, post: &Post) -> Result<(), WebError> {
        let created_at = DateTime::parse_from_rfc3339(&post.indexedAt)
            .map_err(|e| WebError::WTFError(format!("Date parse error: {}", e)))?
            .with_timezone(&Utc);
            
        let default_lang = "fr".to_string();
        let lang = post.record.langs.first().unwrap_or(&default_lang);
        let tweet_id = Self::extract_tweet_id(&post.uri);
        
        // Insérer le tweet principal
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
        
        // Gérer les réponses
        if let Some(reply) = &post.record.reply {
            let parent_tweet_id = Self::extract_tweet_id(&reply.parent.uri);
            
            // Insérer un tweet parent minimal s'il n'existe pas
            // Note: L'API Bluesky ne fournit pas les stats du parent, donc on utilise des valeurs par défaut
            self.insert_basic_tweet(
                &parent_tweet_id,
                created_at,
                &post.author, // On utilise l'auteur de la réponse en fallback
                &format!("Tweet parent ({})", reply.parent.uri),
                "Bluesky",
                "fr",
                0, 0, 0 // Pas de stats disponibles pour le parent
            ).await?;
            
            // Insérer la référence de réponse
            sqlx::query(
                &format!(r#"
                INSERT INTO {}.reply (tweet_id, in_reply_to_tweet_id, in_reply_to_user_id, in_reply_to_screen_name)
                VALUES ($1, $2, $3, $4)
                ON CONFLICT (tweet_id) DO NOTHING
                "#, self.schema_name)
            )
            .bind(&tweet_id)
            .bind(&parent_tweet_id)
            .bind(&post.author.did)
            .bind(&post.author.handle)
            .execute(&self.pool)
            .await
            .map_err(|e| WebError::WTFError(format!("DB insert reply error: {}", e)))?;
        }
        
        // Gérer les embeds (citations potentielles et médias)
        if let Some(embed_value) = &post.embed {
            // Vérifier si c'est un embed de record (citation)
            if let Some(record) = embed_value.get("record") {
                if let Some(record_data) = record.get("record") {
                    if let Some(uri) = record_data.get("uri") {
                        if let Some(uri_str) = uri.as_str() {
                            let quoted_tweet_id = Self::extract_tweet_id(uri_str);
                            
                            // Récupérer le texte du tweet cité si disponible
                            let quoted_text = record_data.get("text")
                                .and_then(|t| t.as_str())
                                .unwrap_or("Tweet cité");
                            
                            // Récupérer l'auteur du tweet cité si disponible
                            let quoted_author_did = record_data.get("author")
                                .and_then(|a| a.get("did"))
                                .and_then(|d| d.as_str())
                                .unwrap_or(&post.author.did);
                            let quoted_author_handle = record_data.get("author")
                                .and_then(|a| a.get("handle"))
                                .and_then(|h| h.as_str())
                                .unwrap_or(&post.author.handle);
                            let quoted_author_name = record_data.get("author")
                                .and_then(|a| a.get("displayName"))
                                .and_then(|n| n.as_str());
                            
                            let quoted_author = Author {
                                did: quoted_author_did.to_string(),
                                handle: quoted_author_handle.to_string(),
                                display_name: quoted_author_name.map(|s| s.to_string()),
                            };
                            
                            // Insérer un tweet cité avec les vraies données si disponibles
                            self.insert_basic_tweet(
                                &quoted_tweet_id,
                                created_at,
                                &quoted_author,
                                quoted_text,
                                "Bluesky",
                                "fr",
                                0, 0, 0 // Pas de stats disponibles pour le tweet cité
                            ).await?;
                            
                            // Insérer la référence de citation
                            sqlx::query(
                                &format!(r#"
                                INSERT INTO {}.quote (tweet_id, quoted_tweet_id)
                                VALUES ($1, $2)
                                ON CONFLICT (tweet_id) DO NOTHING
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
            
            // Insérer les médias
            self.insert_media(&tweet_id, embed_value).await?;
        }
        
        // Insérer les hashtags, URLs et autres éléments depuis les facets
        if let Some(facets) = &post.record.facets {
            self.insert_hashtags(&tweet_id, facets).await?;
            self.insert_urls(&tweet_id, facets).await?;
        }
        
        // Insérer les emojis et cashtags depuis le texte
        self.insert_emojis(&tweet_id, &post.record.text).await?;
        self.insert_cashtags(&tweet_id, &post.record.text).await?;
        
        // Insérer le corpus (texte pour analyse)
                                sqlx::query(
                                    &format!(r#"
            INSERT INTO {}.corpus (tweet_id, corpus)
                                    VALUES ($1, $2)
            ON CONFLICT (tweet_id) DO NOTHING
                                    "#, self.schema_name)
                                )
                                .bind(&tweet_id)
        .bind(&post.record.text)
                                .execute(&self.pool)
                                .await
        .map_err(|e| WebError::WTFError(format!("DB insert corpus error: {}", e)))?;
        
        Ok(())
    }


}

struct TwitterCollector {
    client: reqwest::Client,
    pool: sqlx::PgPool,
    schema_name: String,
}

impl TwitterCollector {
    async fn new(bearer_token: &str, schema_name: String) -> Result<Self, WebError> {
        // Create client with Bearer token
        let mut headers = ReqwestHeaderMap::new();
        let auth = format!("Bearer {}", bearer_token);
        headers.insert(
            AUTHORIZATION, 
            HeaderValue::from_str(&auth).map_err(|e| WebError::WTFError(format!("Header value error: {}", e)))?
        );
        
        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .map_err(|e| WebError::WTFError(format!("Client build error: {}", e)))?;

        // Use the existing PG connection
        let pool = sqlx::PgPool::connect(&std::env::var("PG_DATABASE_URL")
            .unwrap_or_else(|_| "postgres://cocktailuser:cocktailuser@localhost:5432/cocktail_pg".to_string()))
            .await
            .map_err(|e| WebError::WTFError(format!("DB connection error: {}", e)))?;

        Ok(Self { client, pool, schema_name })
    }

    async fn search_tweets(&self, keyword: &str, limit: usize, start_date: Option<&str>, end_date: Option<&str>) -> Result<TwitterSearchResponse, WebError> {
        tracing::info!("Starting complete Twitter search for keyword: '{}', limit: {}", keyword, limit);
        
        // Détecter si on a besoin de l'endpoint /search/all (API Pro) ou /search/recent (gratuit)
        let use_full_archive = if let Some(start) = start_date {
            if let Ok(start_dt) = DateTime::parse_from_rfc3339(start) {
                let seven_days_ago = Utc::now() - chrono::Duration::days(7);
                start_dt.with_timezone(&Utc) < seven_days_ago
            } else {
                false
            }
        } else {
            false
        };
        
        let api_url = if use_full_archive {
            tracing::info!("Using Twitter API /search/all endpoint for historical data");
            TWITTER_API_URL_ALL
        } else {
            tracing::info!("Using Twitter API /search/recent endpoint for recent data");
            TWITTER_API_URL_RECENT
        };
        
        let mut tweets = Vec::new();
        let mut all_users = Vec::new();
        let mut all_places = Vec::new();
        let mut all_media = Vec::new();
        let mut all_referenced_tweets = Vec::new();
        let mut next_token: Option<String> = None;
        let mut request_count = 0;
        let max_requests = 25; // Augmenter à 25 requêtes pour permettre 1000+ tweets (25 * 50 = 1250 tweets max)
        
        while tweets.len() < limit && request_count < max_requests {
            let batch_limit = (limit - tweets.len()).min(100); // Augmenter à 100 tweets par requête (max API Twitter)
            
            // Version complète avec TOUS les champs pour remplir toute la BDD
            let mut params = vec![
                ("query", keyword.to_string()),
                ("max_results", batch_limit.to_string()),
                // TOUS les champs tweets disponibles
                ("tweet.fields", "id,text,created_at,author_id,context_annotations,entities,geo,in_reply_to_user_id,lang,possibly_sensitive,public_metrics,referenced_tweets,reply_settings,source,withheld".to_string()),
                // TOUS les champs utilisateur disponibles
                ("user.fields", "id,name,username,created_at,description,location,pinned_tweet_id,profile_image_url,protected,public_metrics,url,verified,withheld".to_string()),
                // TOUS les champs place disponibles
                ("place.fields", "id,full_name,name,country,country_code,geo,place_type".to_string()),
                // TOUS les champs media disponibles
                ("media.fields", "media_key,type,url,duration_ms,height,preview_image_url,public_metrics,width,alt_text".to_string()),
                // TOUTES les expansions pour avoir le maximum de données
                ("expansions", "author_id,referenced_tweets.id,in_reply_to_user_id,geo.place_id,entities.mentions.username,referenced_tweets.id.author_id,attachments.media_keys".to_string()),
            ];
            
            // Ajouter les paramètres de date si fournis
            // Note: L'API Twitter gratuite ne permet que les 7 derniers jours
            if let Some(start) = start_date {
                // Valider que la date n'est pas trop ancienne (7 jours max pour l'API gratuite)
                if let Ok(start_dt) = DateTime::parse_from_rfc3339(start) {
                    let seven_days_ago = Utc::now() - chrono::Duration::days(7);
                    if start_dt.with_timezone(&Utc) < seven_days_ago {
                        tracing::warn!("start_time {} is older than 7 days, Twitter API may reject it", start);
                        if !use_full_archive {
                            return Err(WebError::WTFError(format!(
                                "ERREUR: Vous tentez de collecter des données du {} mais l'API Twitter gratuite ne permet que les 7 derniers jours. Pour des données historiques, vous avez besoin d'un compte Twitter API Pro/Enterprise avec accès à l'endpoint /search/all.",
                                start
                            )));
                        }
                    }
                }
                params.push(("start_time", start.to_string()));
                tracing::info!("Using start_time: {}", start);
            }
            if let Some(end) = end_date {
                if let Ok(end_dt) = DateTime::parse_from_rfc3339(end) {
                    let seven_days_ago = Utc::now() - chrono::Duration::days(7);
                    if end_dt.with_timezone(&Utc) < seven_days_ago {
                        tracing::warn!("end_time {} is older than 7 days, Twitter API may reject it", end);
                    }
                }
                params.push(("end_time", end.to_string()));
                tracing::info!("Using end_time: {}", end);
            }
            
            if let Some(token) = next_token.as_ref() {
                params.push(("next_token", token.to_string()));
                tracing::info!("Using pagination token: {}", token);
            }
            
            tracing::info!("Twitter API request #{}/{} - Demandé: {} tweets, Collecté: {}/{} tweets", 
                request_count + 1, max_requests, batch_limit, tweets.len(), limit);
            
            let start_time = std::time::Instant::now();
            let response = self.client
                .get(api_url)
                .query(&params)
                .send()
                .await
                .map_err(|e| WebError::WTFError(format!("Twitter API request error: {}", e)))?;
            
            let request_duration = start_time.elapsed();
            tracing::info!("Requête API terminée en {:?}", request_duration);
            
            if !response.status().is_success() {
                let status = response.status();
                let error_text = response.text().await.unwrap_or_default();
                tracing::error!("Twitter API error {}: {}", status, error_text);
                
                // Message d'erreur spécifique pour les problèmes de dates
                if status == 400 && error_text.contains("start_time") {
                    if use_full_archive {
                        return Err(WebError::WTFError(format!(
                            "Erreur d'accès à l'archive complète : Votre token API ne semble pas avoir accès à l'endpoint /search/all. Vérifiez que vous utilisez bien un Bearer Token d'API Twitter Pro/Enterprise."
                        )));
                    } else {
                        return Err(WebError::WTFError(format!(
                            "Erreur de plage de dates : L'API Twitter gratuite ne permet de récupérer que les tweets des 7 derniers jours. Veuillez ajuster vos dates ou laisser les champs vides pour une collecte récente."
                        )));
                    }
                }
                
                return Err(WebError::WTFError(format!("Twitter API error {}: {}", status, error_text)));
            }
            
            let parse_start = std::time::Instant::now();
            let search_response: TwitterSearchResponse = response
                .json()
                .await
                .map_err(|e| WebError::WTFError(format!("Twitter API response parse error: {}", e)))?;
            
            let parse_duration = parse_start.elapsed();
            tracing::info!("Parsing JSON terminé en {:?}", parse_duration);
            
            if let Some(data) = search_response.data {
                if data.is_empty() {
                    tracing::info!("No more tweets found, stopping search");
                    break;
                }
                let new_tweets = data.len();
                tweets.extend(data);
                tracing::info!("Ajouté {} nouveaux tweets (total: {}/{})", new_tweets, tweets.len(), limit);
            } else {
                tracing::info!("No data in response, stopping search");
                break;
            }
            
            // Accumuler tous les includes pour avoir toutes les données
            if let Some(includes) = search_response.includes {
                if let Some(users) = includes.users {
                    let user_count = users.len();
                    all_users.extend(users);
                    tracing::debug!("Ajouté {} utilisateurs aux includes", user_count);
                }
                if let Some(places) = includes.places {
                    let place_count = places.len();
                    all_places.extend(places);
                    tracing::debug!("Ajouté {} lieux aux includes", place_count);
                }
                if let Some(media) = includes.media {
                    let media_count = media.len();
                    all_media.extend(media);
                    tracing::debug!("Ajouté {} médias aux includes", media_count);
                }
                if let Some(ref_tweets) = includes.tweets {
                    let ref_count = ref_tweets.len();
                    all_referenced_tweets.extend(ref_tweets);
                    tracing::debug!("Ajouté {} tweets référencés aux includes", ref_count);
                }
            }
            
            next_token = search_response.meta.and_then(|m| m.next_token);
            request_count += 1;
            
            if next_token.is_none() {
                tracing::info!("No next token, stopping search");
                break;
            }
            
            // Réduire la pause entre les requêtes pour accélérer
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
        
        let final_count = tweets.len().min(limit);
        tracing::info!("Twitter search completed: {} tweets collected for keyword '{}'", final_count, keyword);
        
        // Retourner une réponse complète avec TOUS les includes accumulés
        let complete_includes = if !all_users.is_empty() || !all_places.is_empty() || !all_media.is_empty() || !all_referenced_tweets.is_empty() {
            Some(TwitterIncludes {
                users: if !all_users.is_empty() { Some(all_users) } else { None },
                places: if !all_places.is_empty() { Some(all_places) } else { None },
                media: if !all_media.is_empty() { Some(all_media) } else { None },
                tweets: if !all_referenced_tweets.is_empty() { Some(all_referenced_tweets) } else { None },
            })
        } else {
            None
        };
        
        Ok(TwitterSearchResponse {
            data: Some(tweets.into_iter().take(limit).collect()),
            includes: complete_includes,
            meta: None,
        })
    }

    // Helper pour insérer un utilisateur depuis l'API Twitter
    async fn insert_twitter_user(&self, user: &TwitterUser) -> Result<(), WebError> {
        let created_at = user.created_at.as_deref().unwrap_or("2006-03-21"); // Date de création de Twitter
        
        sqlx::query(
            &format!(r#"
            INSERT INTO {}.user (
                id, screen_name, name, created_at, verified, protected
            ) VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (id) DO NOTHING
            "#, self.schema_name)
        )
        .bind(&user.id)
        .bind(&user.username)
        .bind(&user.name)
        .bind(created_at)
        .bind(user.verified.unwrap_or(false))
        .bind(user.protected.unwrap_or(false))
        .execute(&self.pool)
        .await
        .map_err(|e| WebError::WTFError(format!("DB insert Twitter user error: {}", e)))?;
        
        Ok(())
    }

    // Helper pour insérer un utilisateur fantôme quand on n'a pas les données complètes
    async fn insert_phantom_user(&self, user_id: &str) -> Result<(), WebError> {
        sqlx::query(
            &format!(r#"
            INSERT INTO {}.user (
                id, screen_name, name, created_at, verified, protected
            ) VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (id) DO NOTHING
            "#, self.schema_name)
        )
        .bind(user_id)
        .bind(&format!("user_{}", user_id)) // screen_name par défaut
        .bind(&format!("User {}", user_id)) // name par défaut
        .bind("2006-03-21") // Date de création de Twitter par défaut
        .bind(false) // verified
        .bind(false) // protected
        .execute(&self.pool)
        .await
        .map_err(|e| WebError::WTFError(format!("DB insert phantom user error: {}", e)))?;
        
        Ok(())
    }

    // Helper pour insérer un tweet de base depuis l'API Twitter
    async fn insert_twitter_tweet(&self, tweet: &TwitterTweet, author: Option<&TwitterUser>) -> Result<(), WebError> {
        let created_at = DateTime::parse_from_rfc3339(&tweet.created_at)
            .map_err(|e| WebError::WTFError(format!("Date parse error: {}", e)))?
            .with_timezone(&Utc);
        
        // Si on a les données de l'auteur, les insérer
        if let Some(user) = author {
            self.insert_twitter_user(user).await?;
        } else {
            // Créer un utilisateur fantôme avec l'ID du tweet
            self.insert_phantom_user(&tweet.author_id).await?;
        }
        
                 let metrics = tweet.public_metrics.as_ref();
        let lang = tweet.lang.as_deref().unwrap_or("fr");
        let source = tweet.source.as_deref().unwrap_or("Twitter");
        
        sqlx::query(
            &format!(r#"
            INSERT INTO {}.tweet (
                id, created_at, published_time, user_id, user_name,
                user_screen_name, text, source, language,
                coordinates_longitude, coordinates_latitude, possibly_sensitive,
                retweet_count, reply_count, quote_count
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
            ON CONFLICT (id) DO NOTHING
            "#, self.schema_name)
        )
        .bind(&tweet.id)
        .bind(created_at.format("%Y-%m-%d").to_string())
        .bind(created_at.timestamp_millis())
        .bind(&tweet.author_id)
        .bind(author.map(|u| &u.name).unwrap_or(&"Unknown".to_string()))
        .bind(author.map(|u| &u.username).unwrap_or(&"unknown".to_string()))
        .bind(&tweet.text)
        .bind(source)
        .bind(lang)
        .bind::<Option<String>>(None) // coordinates_longitude
        .bind::<Option<String>>(None) // coordinates_latitude
        .bind(tweet.possibly_sensitive.unwrap_or(false))
        .bind(metrics.map(|m| m.retweet_count).unwrap_or(0))
        .bind(metrics.map(|m| m.reply_count).unwrap_or(0))
        .bind(metrics.map(|m| m.quote_count).unwrap_or(0))
        .execute(&self.pool)
        .await
        .map_err(|e| WebError::WTFError(format!("DB insert Twitter tweet error: {}", e)))?;
        
        Ok(())
    }

    // Helper pour insérer des hashtags depuis l'API Twitter
    async fn insert_twitter_hashtags(&self, tweet_id: &str, hashtags: &[TwitterHashtag]) -> Result<(), WebError> {
        for (order, hashtag) in hashtags.iter().enumerate() {
            sqlx::query(
                &format!(r#"
                INSERT INTO {}.tweet_hashtag (tweet_id, hashtag, "order", start_indice, end_indice)
                VALUES ($1, $2, $3, $4, $5)
                ON CONFLICT (tweet_id, hashtag) DO NOTHING
                "#, self.schema_name)
            )
            .bind(tweet_id)
            .bind(&hashtag.tag)
            .bind(order as i32)
            .bind(hashtag.start)
            .bind(hashtag.end)
            .execute(&self.pool)
            .await
            .map_err(|e| WebError::WTFError(format!("DB insert Twitter hashtag error: {}", e)))?;
        }
        Ok(())
    }

    // Helper pour insérer des URLs depuis l'API Twitter
    async fn insert_twitter_urls(&self, tweet_id: &str, urls: &[TwitterUrl]) -> Result<(), WebError> {
        for (order, url) in urls.iter().enumerate() {
            let final_url = url.expanded_url.as_ref().unwrap_or(&url.url);
            
            sqlx::query(
                &format!(r#"
                INSERT INTO {}.tweet_url (tweet_id, url, "order", start_indice, end_indice)
                VALUES ($1, $2, $3, $4, $5)
                ON CONFLICT (tweet_id, url) DO NOTHING
                "#, self.schema_name)
            )
            .bind(tweet_id)
            .bind(final_url)
            .bind(order as i32)
            .bind(url.start)
            .bind(url.end)
            .execute(&self.pool)
            .await
            .map_err(|e| WebError::WTFError(format!("DB insert Twitter URL error: {}", e)))?;
        }
        Ok(())
    }

    // Helper pour insérer des cashtags depuis l'API Twitter
    async fn insert_twitter_cashtags(&self, tweet_id: &str, cashtags: &[TwitterCashtag]) -> Result<(), WebError> {
        for (order, cashtag) in cashtags.iter().enumerate() {
            sqlx::query(
                &format!(r#"
                INSERT INTO {}.tweet_cashtag (tweet_id, cashtag, "order", start_indice, end_indice)
                VALUES ($1, $2, $3, $4, $5)
                ON CONFLICT (tweet_id, cashtag) DO NOTHING
                "#, self.schema_name)
            )
            .bind(tweet_id)
            .bind(&format!("${}", cashtag.tag))
            .bind(order as i32)
            .bind(cashtag.start)
            .bind(cashtag.end)
            .execute(&self.pool)
            .await
            .map_err(|e| WebError::WTFError(format!("DB insert Twitter cashtag error: {}", e)))?;
        }
        Ok(())
    }

    // Helper pour insérer des mentions d'utilisateurs depuis l'API Twitter
    async fn insert_twitter_mentions(&self, tweet_id: &str, mentions: &[TwitterMention]) -> Result<(), WebError> {
        for (order, mention) in mentions.iter().enumerate() {
            // Si on a l'ID de l'utilisateur mentionné, l'utiliser, sinon utiliser le username
            let user_id = mention.id.as_ref().unwrap_or(&mention.username);
            
            sqlx::query(
                &format!(r#"
                INSERT INTO {}.tweet_user_mention (tweet_id, user_id, "order", start_indice, end_indice)
                VALUES ($1, $2, $3, $4, $5)
                ON CONFLICT (tweet_id, user_id) DO NOTHING
                "#, self.schema_name)
            )
            .bind(tweet_id)
            .bind(user_id)
            .bind(order as i32)
            .bind(mention.start)
            .bind(mention.end)
            .execute(&self.pool)
            .await
            .map_err(|e| WebError::WTFError(format!("DB insert Twitter mention error: {}", e)))?;
        }
        Ok(())
    }

    // Helper pour insérer des médias depuis l'API Twitter
    async fn insert_twitter_media(&self, tweet_id: &str, media_list: &[TwitterMedia]) -> Result<(), WebError> {
        for (order, media) in media_list.iter().enumerate() {
            let media_url = media.url.as_deref()
                .or(media.preview_image_url.as_deref())
                .unwrap_or(&media.media_key);
            
            sqlx::query(
                &format!(r#"
                INSERT INTO {}.tweet_media (tweet_id, media_url, type, "order")
                VALUES ($1, $2, $3, $4)
                ON CONFLICT (tweet_id, media_url) DO NOTHING
                "#, self.schema_name)
            )
            .bind(tweet_id)
            .bind(media_url)
            .bind(&media.media_type)
            .bind(order as i32)
            .execute(&self.pool)
            .await
            .map_err(|e| WebError::WTFError(format!("DB insert Twitter media error: {}", e)))?;
        }
        Ok(())
    }

    // Helper pour insérer un lieu depuis l'API Twitter
    async fn insert_twitter_place(&self, place: &TwitterPlace) -> Result<(), WebError> {
        let bbox_str = place.geo.as_ref()
            .and_then(|g| g.bbox.as_ref())
            .map(|bbox| format!("{:?}", bbox));
            
        sqlx::query(
            &format!(r#"
            INSERT INTO {}.place (
                id, name, full_name, country_code, country, place_type, 
                url, bounding_box, type_bounding_box
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (id) DO NOTHING
            "#, self.schema_name)
        )
        .bind(&place.id)
        .bind(&place.name)
        .bind(&place.full_name)
        .bind(place.country_code.as_deref())
        .bind(place.country.as_deref())
        .bind(place.place_type.as_deref())
        .bind::<Option<String>>(None) // URL pas disponible dans l'API v2
        .bind(bbox_str.as_deref())
        .bind(place.geo.as_ref().map(|g| g.geo_type.as_str()))
        .execute(&self.pool)
        .await
        .map_err(|e| WebError::WTFError(format!("DB insert Twitter place error: {}", e)))?;
        
        Ok(())
    }

    // Helper pour insérer la liaison tweet-place
    async fn insert_twitter_tweet_place(&self, tweet_id: &str, place_id: &str) -> Result<(), WebError> {
        sqlx::query(
            &format!(r#"
            INSERT INTO {}.tweet_place (tweet_id, place_id)
            VALUES ($1, $2)
            ON CONFLICT (tweet_id, place_id) DO NOTHING
            "#, self.schema_name)
        )
        .bind(tweet_id)
        .bind(place_id)
        .execute(&self.pool)
        .await
        .map_err(|e| WebError::WTFError(format!("DB insert Twitter tweet_place error: {}", e)))?;
        
        Ok(())
    }

    // Helper pour mettre à jour les coordonnées d'un tweet
    async fn update_tweet_coordinates(&self, tweet_id: &str, coordinates: &TwitterCoordinates) -> Result<(), WebError> {
        if coordinates.coordinates.len() >= 2 {
            sqlx::query(
                &format!(r#"
                UPDATE {}.tweet 
                SET coordinates_longitude = $1, coordinates_latitude = $2
                WHERE id = $3
                "#, self.schema_name)
            )
            .bind(coordinates.coordinates[0].to_string())
            .bind(coordinates.coordinates[1].to_string())
            .bind(tweet_id)
            .execute(&self.pool)
            .await
            .map_err(|e| WebError::WTFError(format!("DB update tweet coordinates error: {}", e)))?;
        }
        
        Ok(())
    }

    // Helper pour insérer les pays withheld
    async fn insert_withheld_country(&self, user_id: &str, country: &str) -> Result<(), WebError> {
        sqlx::query(
            &format!(r#"
            INSERT INTO {}.withheld_in_country (user_id, country)
            VALUES ($1, $2)
            ON CONFLICT (user_id, country) DO NOTHING
            "#, self.schema_name)
        )
        .bind(user_id)
        .bind(country)
        .execute(&self.pool)
        .await
        .map_err(|e| WebError::WTFError(format!("DB insert withheld country error: {}", e)))?;
        
        Ok(())
    }

    // Helper pour extraire et insérer les emojis depuis le texte
    async fn insert_emoji_from_text(&self, tweet_id: &str, text: &str) -> Result<(), WebError> {
        // Regex pour capturer les emojis communs
        let emoji_regex = Regex::new(r"[\u{1F600}-\u{1F64F}]|[\u{1F300}-\u{1F5FF}]|[\u{1F680}-\u{1F6FF}]|[\u{1F1E0}-\u{1F1FF}]|[\u{2600}-\u{26FF}]|[\u{2700}-\u{27BF}]").unwrap();
        let mut order = 0;
        
        for emoji_match in emoji_regex.find_iter(text) {
            sqlx::query(
                &format!(r#"
                INSERT INTO {}.tweet_emoji (tweet_id, emoji, "order", start_indice, end_indice)
                VALUES ($1, $2, $3, $4, $5)
                ON CONFLICT (tweet_id, emoji) DO NOTHING
                "#, self.schema_name)
            )
            .bind(tweet_id)
            .bind(emoji_match.as_str())
            .bind(order)
            .bind(emoji_match.start() as i32)
            .bind(emoji_match.end() as i32)
            .execute(&self.pool)
            .await
            .map_err(|e| WebError::WTFError(format!("DB insert emoji error: {}", e)))?;
            
            order += 1;
        }
        Ok(())
    }

    async fn save_tweet_to_db(&self, tweet: &TwitterTweet, includes: Option<&TwitterIncludes>) -> Result<(), WebError> {
        // D'abord, insérer tous les tweets référencés depuis les includes
        if let Some(includes) = includes {
            if let Some(referenced_tweets) = &includes.tweets {
                for ref_tweet in referenced_tweets {
                    // Trouver l'auteur du tweet référencé
                    let ref_author = includes.users.as_ref()
                        .and_then(|users| users.iter().find(|u| u.id == ref_tweet.author_id));
                    
                    // Insérer le tweet référencé (parent/cité/retweeté) d'abord
                    self.insert_twitter_tweet(ref_tweet, ref_author).await?;
                    
                    tracing::debug!("Inserted referenced tweet {} from includes", ref_tweet.id);
                }
            }
        }
        
        // Trouver l'auteur du tweet principal dans les includes
        let author = includes
            .and_then(|inc| inc.users.as_ref())
            .and_then(|users| users.iter().find(|u| u.id == tweet.author_id));
        
        // Insérer le tweet principal
        self.insert_twitter_tweet(tweet, author).await?;
        
        // Gérer les réponses
        if let Some(in_reply_to_user_id) = &tweet.in_reply_to_user_id {
            // Chercher le tweet parent dans referenced_tweets
            if let Some(referenced_tweets) = &tweet.referenced_tweets {
                for ref_tweet in referenced_tweets {
                    if ref_tweet.ref_type == "replied_to" {
                        // Insérer la référence de réponse
                        sqlx::query(
                            &format!(r#"
                            INSERT INTO {}.reply (tweet_id, in_reply_to_tweet_id, in_reply_to_user_id, in_reply_to_screen_name)
                            VALUES ($1, $2, $3, $4)
                            ON CONFLICT (tweet_id) DO NOTHING
                            "#, self.schema_name)
                        )
                        .bind(&tweet.id)
                        .bind(&ref_tweet.id)
                        .bind(in_reply_to_user_id)
                        .bind("unknown") // Le nom d'utilisateur n'est pas toujours disponible
                        .execute(&self.pool)
                        .await
                        .map_err(|e| WebError::WTFError(format!("DB insert Twitter reply error: {}", e)))?;
                        
                        tracing::info!("Created reply relation: {} -> {}", tweet.id, ref_tweet.id);
                        break;
                    }
                }
            }
        }
        
        // Gérer les retweets et citations
        if let Some(referenced_tweets) = &tweet.referenced_tweets {
            for ref_tweet in referenced_tweets {
                match ref_tweet.ref_type.as_str() {
                    "retweeted" => {
                        sqlx::query(
                            &format!(r#"
                            INSERT INTO {}.retweet (tweet_id, retweeted_tweet_id)
                            VALUES ($1, $2)
                            ON CONFLICT (tweet_id) DO NOTHING
                            "#, self.schema_name)
                        )
                        .bind(&tweet.id)
                        .bind(&ref_tweet.id)
                        .execute(&self.pool)
                        .await
                        .map_err(|e| WebError::WTFError(format!("DB insert Twitter retweet error: {}", e)))?;
                        
                        tracing::info!("Created retweet relation: {} -> {}", tweet.id, ref_tweet.id);
                    },
                    "quoted" => {
                        sqlx::query(
                            &format!(r#"
                            INSERT INTO {}.quote (tweet_id, quoted_tweet_id)
                            VALUES ($1, $2)
                            ON CONFLICT (tweet_id) DO NOTHING
                            "#, self.schema_name)
                        )
                        .bind(&tweet.id)
                        .bind(&ref_tweet.id)
                        .execute(&self.pool)
                        .await
                        .map_err(|e| WebError::WTFError(format!("DB insert Twitter quote error: {}", e)))?;
                        
                        tracing::info!("Created quote relation: {} -> {}", tweet.id, ref_tweet.id);
                    },
                    _ => {
                        tracing::debug!("Unknown reference type: {}", ref_tweet.ref_type);
                    }
                }
            }
        }
        
        // Insérer les entités (hashtags, URLs, mentions, cashtags)
        if let Some(entities) = &tweet.entities {
            if let Some(hashtags) = &entities.hashtags {
                self.insert_twitter_hashtags(&tweet.id, hashtags).await?;
            }
            
            if let Some(urls) = &entities.urls {
                self.insert_twitter_urls(&tweet.id, urls).await?;
            }
            
            if let Some(cashtags) = &entities.cashtags {
                self.insert_twitter_cashtags(&tweet.id, cashtags).await?;
            }
            
            if let Some(mentions) = &entities.mentions {
                self.insert_twitter_mentions(&tweet.id, mentions).await?;
            }
        }
        
        // Insérer les médias s'il y en a
        if let Some(includes) = includes {
            if let Some(media_list) = &includes.media {
                self.insert_twitter_media(&tweet.id, media_list).await?;
            }
            
            // Insérer les lieux s'il y en a
            if let Some(places) = &includes.places {
                for place in places {
                    // D'abord insérer le lieu dans la table place
                    self.insert_twitter_place(place).await?;
                    
                    // Si le tweet fait référence à ce lieu, créer la liaison
                    if let Some(geo) = &tweet.geo {
                        if let Some(place_id) = &geo.place_id {
                            if place_id == &place.id {
                                self.insert_twitter_tweet_place(&tweet.id, &place.id).await?;
                            }
                        }
                    }
                }
            }
        }
        
        // Insérer les coordonnées géographiques dans le tweet si disponibles
        if let Some(geo) = &tweet.geo {
            if let Some(coordinates) = &geo.coordinates {
                self.update_tweet_coordinates(&tweet.id, coordinates).await?;
            }
        }
        
        // Insérer les informations withheld si disponibles
        if let Some(withheld) = &tweet.withheld {
            if let Some(country_codes) = &withheld.country_codes {
                for country in country_codes {
                    self.insert_withheld_country(&tweet.author_id, country).await?;
                }
            }
        }
        
        // Extraire et insérer les emojis du texte
        self.insert_emoji_from_text(&tweet.id, &tweet.text).await?;
        
        // Insérer le corpus (texte pour analyse)
        sqlx::query(
            &format!(r#"
            INSERT INTO {}.corpus (tweet_id, corpus)
            VALUES ($1, $2)
            ON CONFLICT (tweet_id) DO NOTHING
            "#, self.schema_name)
        )
        .bind(&tweet.id)
        .bind(&tweet.text)
        .execute(&self.pool)
        .await
        .map_err(|e| WebError::WTFError(format!("DB insert Twitter corpus error: {}", e)))?;
        
        Ok(())
    }
}

// Handler for the collect page - utilise maintenant le bon template
pub async fn collect(
    paths: ProjectCollect,
    authuser: AuthenticatedUser,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, WebError> {
    let logout_url = get_logout_url(state.kratos_configuration, headers).await;
    let project = cocktail_db_web::project(&state.db, paths.project_id.to_hyphenated(), &authuser.user_id).await?;
    let (include_count, exclude_count) =
        cocktail_db_web::include_exclude_hashtag_count(&state.db, paths.project_id.to_hyphenated(), &authuser.user_id)
            .await?;
    
    Ok(HtmlTemplate(templates::Collect {
        daterange_path: ProjectDateRange { project_id: paths.project_id },
        hashtag_path: ProjectHashtags { project_id: paths.project_id },
        request_path: ProjectRequest { project_id: paths.project_id },
        collect_path: ProjectCollect { project_id: paths.project_id },
        import_path: ProjectImport { project_id: paths.project_id },
        delete_popup_path: PopupDeleteProject { project_id: paths.project_id },
        rename_popup_path: PopupRenameProject { project_id: paths.project_id },
        duplicate_popup_path: PopupDuplicateProject { project_id: paths.project_id },
        download_path: DownloadProject { project_id: paths.project_id },
        analysis_preview_popup_path: PopupAnalysisPreview { project_id: paths.project_id },
        analysis_path: ProjectAnalysis { project_id: paths.project_id },
        is_analyzed: project.is_analyzed == 1,
        results_path: ProjectResults { project_id: paths.project_id },
        tweets_graph_path: ProjectTweetsGraph { project_id: paths.project_id },
        authors_path: ProjectAuthors { project_id: paths.project_id },
        result_hashtags_path: ProjectResultHashtags { project_id: paths.project_id },
        communities_path: Communities { project_id: paths.project_id },
        logout_url,
        include_count,
        exclude_count,
        niveau: authuser.niveau,
        last_login_datetime: authuser.last_login_datetime,
        title: project.title,
        tweets_count: project.tweets_count,
        authors_count: project.authors_count,
    }))
}

// Helper function to create tables for a schema
async fn create_collection_tables(pool: &sqlx::PgPool, schema_name: &str) -> Result<(), WebError> {
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

    // Table Corpus (sera créée après Tweet pour respecter les références)
    // Définie plus tard dans le code

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

// Handler for starting a collection
pub async fn start_collection(
    path: StartCollection,
    State(_state): State<AppState>,
    _authuser: AuthenticatedUser,
    Json(req): Json<CollectionRequest>,
) -> Result<impl IntoResponse, WebError> {
    tracing::info!("Starting collection with parameters: {:?}", req);
    
    // Vérification préliminaire pour Twitter API et dates
    if req.networks.contains(&"twitter".to_string()) {
        if let Some(start_date) = &req.start_date {
            if let Ok(start_dt) = DateTime::parse_from_rfc3339(start_date) {
                let seven_days_ago = Utc::now() - chrono::Duration::days(7);
                if start_dt.with_timezone(&Utc) < seven_days_ago {
                    // Vérifier si on a un token qui supporte l'API complète
                    if let Ok(bearer_token) = std::env::var("TWITTER_BEARER_TOKEN") {
                        if !bearer_token.starts_with("AAAA") {
                            return Err(WebError::WTFError(format!(
                                "ERREUR: Vous tentez de collecter des données depuis le {} mais votre token Twitter API ne semble pas supporter l'archive complète. L'API Twitter gratuite ne permet que les 7 derniers jours. Pour des données historiques, vous avez besoin d'un compte Twitter API Pro/Enterprise.",
                                start_date
                            )));
                        }
                        tracing::info!("Détection de dates historiques, tentative d'utilisation de l'API Twitter complète");
                    } else {
                        return Err(WebError::WTFError(
                            "Token Twitter API manquant dans les variables d'environnement".to_string()
                        ));
                    }
                }
            }
        }
    }
    
    // Create schema name based on current date
    let schema_name = format!("collect_{}", Local::now().format("%Y%m%d"));
    tracing::info!("Using schema name: {}", schema_name);
    
    // Get database connection
    let pool = sqlx::PgPool::connect(&std::env::var("PG_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://cocktailuser:cocktailuser@localhost:5432/cocktail_pg".to_string()))
        .await
        .map_err(|e| WebError::WTFError(format!("DB connection error: {}", e)))?;

    // Create tables for the schema
    create_collection_tables(&pool, &schema_name).await?;

    let mut total_posts = 0;
    
    // Process each keyword
    for keyword in &req.keywords {
        // Default limit to 10 if not specified
        let limit = req.limit.unwrap_or(10);
        
        // Process each selected network
        for network in &req.networks {
            match network.as_str() {
                "bluesky" => {
                    // Get Bluesky credentials from environment
                    if let (Ok(handle), Ok(app_password)) = (
                        std::env::var("BLUESKY_HANDLE"),
                        std::env::var("BLUESKY_APP_PASSWORD")
                    ) {
                        if let Ok(collector) = BlueskyCollector::new(&handle, &app_password, schema_name.clone()).await {
                            // Search for posts with date range
                            if let Ok(posts) = collector.search_posts(keyword, limit, req.start_date.as_deref(), req.end_date.as_deref()).await {
                                // Save posts to database
                                for post in &posts {
                                    if let Err(e) = collector.save_post_to_db(post).await {
                                        tracing::warn!("Error saving Bluesky post: {}", e);
                                    } else {
                                        total_posts += 1;
                                    }
                                }
                            } else {
                                tracing::warn!("Failed to search Bluesky posts for keyword: {}", keyword);
                            }
                        } else {
                            tracing::warn!("Failed to create Bluesky collector");
                        }
                    } else {
                        tracing::warn!("Bluesky credentials not set, skipping Bluesky collection");
                    }
                },
                "twitter" => {
                    // Get Twitter credentials from environment
                    if let Ok(bearer_token) = std::env::var("TWITTER_BEARER_TOKEN") {
                        match TwitterCollector::new(&bearer_token, schema_name.clone()).await {
                            Ok(collector) => {
                                // Search for tweets with date range
                                match collector.search_tweets(keyword, limit, req.start_date.as_deref(), req.end_date.as_deref()).await {
                                    Ok(search_response) => {
                                        tracing::info!("Found {} Twitter tweets for keyword: {}", search_response.data.as_ref().map(|d| d.len()).unwrap_or(0), keyword);
                                        
                                        // Traiter les tweets avec toutes les données includes
                                        if let Some(tweets) = &search_response.data {
                                            let save_start = std::time::Instant::now();
                                            let mut saved_count = 0;
                                            
                                            for (index, tweet) in tweets.iter().enumerate() {
                                                let tweet_save_start = std::time::Instant::now();
                                                if let Err(e) = collector.save_tweet_to_db(tweet, search_response.includes.as_ref()).await {
                                                    tracing::warn!("Error saving Twitter tweet {}: {}", tweet.id, e);
                                                } else {
                                                    saved_count += 1;
                                                    total_posts += 1;
                                                    
                                                    // Log de progression tous les 10 tweets
                                                    if index % 10 == 0 || index == tweets.len() - 1 {
                                                        let tweet_save_duration = tweet_save_start.elapsed();
                                                        tracing::info!("Tweet {}/{} sauvegardé en {:?} (total sauvegardé: {})", 
                                                            index + 1, tweets.len(), tweet_save_duration, saved_count);
                                                    }
                                                }
                                            }
                                            
                                            let save_duration = save_start.elapsed();
                                            tracing::info!("Sauvegarde de {} tweets terminée en {:?} (avg: {:?}/tweet)", 
                                                saved_count, save_duration, save_duration / saved_count.max(1) as u32);
                                        }
                                    },
                                    Err(e) => {
                                        tracing::warn!("Failed to search Twitter tweets for keyword '{}': {}", keyword, e);
                                    }
                                }
                            },
                            Err(e) => {
                                tracing::warn!("Failed to create Twitter collector: {}", e);
                            }
                        }
                    } else {
                        tracing::warn!("Twitter Bearer Token not set, skipping Twitter collection");
                    }
                },
                _ => {
                    tracing::warn!("Unknown network: {}", network);
                }
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