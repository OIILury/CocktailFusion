use serde::{Deserialize, Serialize};

// Structures pour les requêtes et réponses de l'API
#[derive(Debug, Serialize, Deserialize)]
pub struct CollectionRequest {
    pub name: String,
    pub keywords: Vec<String>,
    pub networks: Vec<String>,
    pub limit: Option<usize>,
    pub start_date: Option<String>, // Format ISO 8601
    pub end_date: Option<String>,   // Format ISO 8601
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CollectionResponse {
    pub success: bool,
    pub message: String,
    pub count: usize,
}

// ===============================
// Structures Bluesky API
// ===============================

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthResponse {
    #[serde(rename = "accessJwt")]
    pub access_jwt: String,
    #[serde(rename = "refreshJwt")]
    pub refresh_jwt: String,
    pub handle: String,
    pub did: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchResponse {
    pub posts: Vec<Post>,
    #[serde(default)]
    pub cursor: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Post {
    pub uri: String,
    pub cid: String,
    pub author: Author,
    pub record: PostRecord,
    #[serde(rename = "replyCount")]
    pub reply_count: i64,
    #[serde(rename = "repostCount")]
    pub repost_count: i64,
    #[serde(rename = "likeCount")]
    pub like_count: i64,
    #[serde(rename = "quoteCount")]
    pub quote_count: i64,
    #[serde(rename = "indexedAt")]
    pub indexed_at: String,
    #[serde(default)]
    pub embed: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PostRecord {
    #[serde(rename = "$type")]
    pub type_field: String,
    pub text: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(default)]
    pub langs: Vec<String>,
    #[serde(default)]
    pub facets: Option<Vec<Facet>>,
    #[serde(default)]
    pub reply: Option<Reply>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Facet {
    pub features: Vec<FacetFeature>,
    pub index: FacetIndex,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FacetFeature {
    #[serde(rename = "$type")]
    pub type_field: String,
    #[serde(default)]
    pub tag: Option<String>,
    #[serde(default)]
    pub uri: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FacetIndex {
    #[serde(rename = "byteStart")]
    pub byte_start: i32,
    #[serde(rename = "byteEnd")]
    pub byte_end: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Author {
    pub did: String,
    pub handle: String,
    #[serde(rename = "displayName")]
    pub display_name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Reply {
    pub parent: ReplyData,
    pub root: ReplyData,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReplyData {
    pub cid: String,
    pub uri: String,
}

// ===============================
// Structures Twitter API
// ===============================

#[derive(Debug, Serialize, Deserialize)]
pub struct TwitterSearchResponse {
    pub data: Option<Vec<TwitterTweet>>,
    pub includes: Option<TwitterIncludes>,
    #[serde(default)]
    pub meta: Option<TwitterMeta>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TwitterTweet {
    pub id: String,
    pub text: String,
    pub created_at: String,
    pub author_id: String,
    #[serde(default)]
    pub context_annotations: Option<Vec<TwitterContextAnnotation>>,
    #[serde(default)]
    pub entities: Option<TwitterEntities>,
    #[serde(default)]
    pub geo: Option<TwitterGeo>,
    #[serde(default)]
    pub in_reply_to_user_id: Option<String>,
    #[serde(default)]
    pub lang: Option<String>,
    #[serde(default)]
    pub possibly_sensitive: Option<bool>,
    pub public_metrics: Option<TwitterPublicMetrics>,
    #[serde(default)]
    pub referenced_tweets: Option<Vec<TwitterReferencedTweet>>,
    #[serde(default)]
    pub reply_settings: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub withheld: Option<TwitterWithheld>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TwitterIncludes {
    #[serde(default)]
    pub users: Option<Vec<TwitterUser>>,
    #[serde(default)]
    pub tweets: Option<Vec<TwitterTweet>>,
    #[serde(default)]
    pub places: Option<Vec<TwitterPlace>>,
    #[serde(default)]
    pub media: Option<Vec<TwitterMedia>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TwitterUser {
    pub id: String,
    pub username: String,
    pub name: String,
    pub created_at: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub location: Option<String>,
    #[serde(default)]
    pub pinned_tweet_id: Option<String>,
    #[serde(default)]
    pub profile_image_url: Option<String>,
    #[serde(default)]
    pub protected: Option<bool>,
    pub public_metrics: Option<TwitterUserPublicMetrics>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub verified: Option<bool>,
    #[serde(default)]
    pub withheld: Option<TwitterWithheld>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TwitterMeta {
    pub newest_id: Option<String>,
    pub oldest_id: Option<String>,
    pub result_count: i32,
    #[serde(default)]
    pub next_token: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TwitterContextAnnotation {
    pub domain: TwitterDomain,
    pub entity: TwitterEntity,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TwitterDomain {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TwitterEntity {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TwitterEntities {
    #[serde(default)]
    pub annotations: Option<Vec<TwitterAnnotation>>,
    #[serde(default)]
    pub cashtags: Option<Vec<TwitterCashtag>>,
    #[serde(default)]
    pub hashtags: Option<Vec<TwitterHashtag>>,
    #[serde(default)]
    pub mentions: Option<Vec<TwitterMention>>,
    #[serde(default)]
    pub urls: Option<Vec<TwitterUrl>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TwitterAnnotation {
    pub start: i32,
    pub end: i32,
    pub probability: f64,
    #[serde(rename = "type")]
    pub annotation_type: String,
    pub normalized_text: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TwitterCashtag {
    pub start: i32,
    pub end: i32,
    pub tag: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TwitterHashtag {
    pub start: i32,
    pub end: i32,
    pub tag: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TwitterMention {
    pub start: i32,
    pub end: i32,
    pub username: String,
    pub id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TwitterUrl {
    pub start: i32,
    pub end: i32,
    pub url: String,
    pub expanded_url: Option<String>,
    pub display_url: Option<String>,
    pub media_key: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TwitterGeo {
    pub coordinates: Option<TwitterCoordinates>,
    pub place_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TwitterCoordinates {
    #[serde(rename = "type")]
    pub coord_type: String,
    pub coordinates: Vec<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TwitterPublicMetrics {
    pub retweet_count: i64,
    pub reply_count: i64,
    pub like_count: i64,
    pub quote_count: i64,
    #[serde(default)]
    pub bookmark_count: Option<i64>,
    #[serde(default)]
    pub impression_count: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TwitterUserPublicMetrics {
    pub followers_count: i64,
    pub following_count: i64,
    pub tweet_count: i64,
    pub listed_count: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TwitterReferencedTweet {
    #[serde(rename = "type")]
    pub ref_type: String, // "retweeted", "quoted", "replied_to"
    pub id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TwitterPlace {
    pub id: String,
    pub full_name: String,
    pub name: String,
    pub country: Option<String>,
    pub country_code: Option<String>,
    pub geo: Option<TwitterPlaceGeo>,
    pub place_type: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TwitterPlaceGeo {
    #[serde(rename = "type")]
    pub geo_type: String,
    pub bbox: Option<Vec<f64>>,
    pub properties: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TwitterMedia {
    pub media_key: String,
    #[serde(rename = "type")]
    pub media_type: String,
    pub url: Option<String>,
    pub duration_ms: Option<i64>,
    pub height: Option<i64>,
    pub preview_image_url: Option<String>,
    pub public_metrics: Option<TwitterMediaMetrics>,
    pub width: Option<i64>,
    pub alt_text: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TwitterMediaMetrics {
    pub view_count: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TwitterWithheld {
    pub copyright: Option<bool>,
    pub country_codes: Option<Vec<String>>,
    pub scope: Option<String>,
} 