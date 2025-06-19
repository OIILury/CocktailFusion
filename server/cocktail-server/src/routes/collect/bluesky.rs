use chrono::{DateTime, Utc};
use reqwest::header::{HeaderMap as ReqwestHeaderMap, HeaderValue, AUTHORIZATION};
use regex::Regex;

use crate::error::WebError;
use super::types::*;
use super::database::{BLUESKY_API_URL, BLUESKY_AUTH_URL};

pub struct BlueskyCollector {
    client: reqwest::Client,
    pool: sqlx::PgPool,
    schema_name: String,
}

impl BlueskyCollector {
    pub async fn new(handle: &str, app_password: &str, schema_name: String) -> Result<Self, WebError> {
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

    pub async fn search_posts(&self, keyword: &str, limit: usize, start_date: Option<&str>, end_date: Option<&str>) -> Result<Vec<Post>, WebError> {
        tracing::info!("Starting optimized Bluesky search for keyword: '{}', limit: {}", keyword, limit);
        
        let mut posts = Vec::new();
        let mut cursor: Option<String> = None;
        
        while posts.len() < limit {
            // Optimisation : batches plus gros pour Bluesky (25 max selon API)
            let batch_limit = (limit - posts.len()).min(25);
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
                    if let Ok(post_date) = DateTime::parse_from_rfc3339(&post.indexed_at) {
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

    // Helper pour insérer des hashtags avec indices - VERSION BATCH
    async fn insert_hashtags(&self, tweet_id: &str, facets: &[Facet]) -> Result<(), WebError> {
        // Collecter d'abord tous les hashtags
        let hashtags: Vec<(&str, i32, i32)> = facets
            .iter()
            .flat_map(|facet| {
                facet.features.iter().filter_map(|feature| {
                    if feature.type_field == "app.bsky.richtext.facet#tag" {
                        feature.tag.as_ref().map(|tag| {
                            (tag.as_str(), facet.index.byte_start, facet.index.byte_end)
                        })
                    } else {
                        None
                    }
                })
            })
            .collect();
        
        if hashtags.is_empty() {
            return Ok(());
        }
        
        // Traiter par lots de 100 pour éviter les arrays trop volumineux
        const BATCH_SIZE: usize = 100;
        for chunk in hashtags.chunks(BATCH_SIZE) {
            let tweet_ids: Vec<&str> = chunk.iter().map(|_| tweet_id).collect();
            let tags: Vec<&str> = chunk.iter().map(|(tag, _, _)| *tag).collect();
            let orders: Vec<i32> = (0..chunk.len()).map(|i| i as i32).collect();
            let starts: Vec<i32> = chunk.iter().map(|(_, start, _)| *start).collect();
            let ends: Vec<i32> = chunk.iter().map(|(_, _, end)| *end).collect();
            
            let query = format!(
                r#"
                INSERT INTO {}.tweet_hashtag (tweet_id, hashtag, "order", start_indice, end_indice)
                SELECT * FROM UNNEST($1::text[], $2::text[], $3::int[], $4::int[], $5::int[])
                ON CONFLICT (tweet_id, hashtag) DO NOTHING
                "#,
                self.schema_name
            );
            
            sqlx::query(&query)
                .bind(tweet_ids)
                .bind(tags)
                .bind(orders)
                .bind(starts)
                .bind(ends)
                .execute(&self.pool)
                .await
                .map_err(|e| WebError::WTFError(format!("DB batch insert hashtags error: {}", e)))?;
        }
        
        Ok(())
    }

    // Helper pour insérer des URLs avec indices - VERSION BATCH
    async fn insert_urls(&self, tweet_id: &str, facets: &[Facet]) -> Result<(), WebError> {
        // Collecter d'abord toutes les URLs
        let urls: Vec<(&str, i32, i32)> = facets
            .iter()
            .flat_map(|facet| {
                facet.features.iter().filter_map(|feature| {
                    if feature.type_field == "app.bsky.richtext.facet#link" {
                        feature.uri.as_ref().map(|uri| {
                            (uri.as_str(), facet.index.byte_start, facet.index.byte_end)
                        })
                    } else {
                        None
                    }
                })
            })
            .collect();
        
        if urls.is_empty() {
            return Ok(());
        }
        
        const BATCH_SIZE: usize = 100;
        for chunk in urls.chunks(BATCH_SIZE) {
            let tweet_ids: Vec<&str> = chunk.iter().map(|_| tweet_id).collect();
            let uris: Vec<&str> = chunk.iter().map(|(uri, _, _)| *uri).collect();
            let orders: Vec<i32> = (0..chunk.len()).map(|i| i as i32).collect();
            let starts: Vec<i32> = chunk.iter().map(|(_, start, _)| *start).collect();
            let ends: Vec<i32> = chunk.iter().map(|(_, _, end)| *end).collect();
            
            let query = format!(
                r#"
                INSERT INTO {}.tweet_url (tweet_id, url, "order", start_indice, end_indice)
                SELECT * FROM UNNEST($1::text[], $2::text[], $3::int[], $4::int[], $5::int[])
                ON CONFLICT (tweet_id, url) DO NOTHING
                "#,
                self.schema_name
            );
            
            sqlx::query(&query)
                .bind(tweet_ids)
                .bind(uris)
                .bind(orders)
                .bind(starts)
                .bind(ends)
                .execute(&self.pool)
                .await
                .map_err(|e| WebError::WTFError(format!("DB batch insert URLs error: {}", e)))?;
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

    // Nouvelle méthode optimisée pour traitement en batch
    pub async fn save_posts_batch_to_db(&self, posts: &[Post]) -> Result<usize, WebError> {
        let mut saved_count = 0;
        
        // Traitement séquentiel mais fiable - évite les deadlocks de transaction
        for post in posts {
            if let Err(e) = self.save_post_to_db(post).await {
                tracing::warn!("Error saving post {} in batch: {}", post.uri, e);
            } else {
                saved_count += 1;
            }
            
            // Log de progression pour voir l'avancement
            if saved_count % 10 == 0 {
                tracing::info!("Saved {}/{} posts...", saved_count, posts.len());
            }
        }
        
        tracing::info!("Batch processing completed: {}/{} posts saved", saved_count, posts.len());
        Ok(saved_count)
    }

    pub async fn save_post_to_db(&self, post: &Post) -> Result<(), WebError> {
        let created_at = DateTime::parse_from_rfc3339(&post.indexed_at)
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
            post.repost_count,
            post.reply_count,
            post.quote_count
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

    // Nouvelle méthode ULTRA SCALABLE pour insérer TOUTES les données de TOUS les posts Bluesky
    pub async fn save_all_posts_ultra_batch(&self, posts: &[Post]) -> Result<usize, WebError> {
        if posts.is_empty() {
            return Ok(0);
        }

        tracing::info!("🚀 Démarrage insertion ultra-scalable Bluesky pour {} posts", posts.len());
        let start_time = std::time::Instant::now();

        // ÉTAPE 1: Users (auteurs des posts)
        let authors: Vec<&Author> = posts.iter().map(|p| &p.author).collect();
        self.bulk_insert_bluesky_users(&authors).await?;
        tracing::info!("✅ Users Bluesky insérés: {}", authors.len());

        // ÉTAPE 2: Posts principaux
        self.bulk_insert_bluesky_posts(posts).await?;
        tracing::info!("✅ Posts Bluesky insérés: {}", posts.len());

        // ÉTAPE 3: Collecter tous les IDs de posts disponibles pour validation des FK
        let mut all_available_post_ids = std::collections::HashSet::new();
        for post in posts {
            let tweet_id = Self::extract_tweet_id(&post.uri);
            all_available_post_ids.insert(tweet_id);
        }

        // ÉTAPE 4: Relations et corpus avec validation FK
        let mut all_corpus = Vec::new();
        let mut all_replies = Vec::new();

        for post in posts {
            let tweet_id = Self::extract_tweet_id(&post.uri);
            
            // Corpus
            all_corpus.push((tweet_id.clone(), post.record.text.clone()));

            // Relations de réponse avec validation FK
            if let Some(reply) = &post.record.reply {
                let parent_id = Self::extract_tweet_id(&reply.parent.uri);
                // Vérifier que le post parent existe dans notre jeu de données
                if all_available_post_ids.contains(&parent_id) {
                    all_replies.push((tweet_id.clone(), parent_id, post.author.did.clone(), "unknown".to_string()));
                } else {
                    tracing::warn!("Post parent {} non trouvé dans le jeu de données, relation ignorée", parent_id);
                }
            }
        }

        self.bulk_insert_bluesky_corpus(&all_corpus).await?;
        self.bulk_insert_bluesky_replies(&all_replies).await?;
        
        tracing::info!("✅ Relations Bluesky insérées: {} corpus, {} replies", 
            all_corpus.len(), all_replies.len());

        // ÉTAPE 5: Entités en masse
        let mut all_hashtags = Vec::new();
        let mut all_urls = Vec::new();
        let mut all_emojis = Vec::new();

        for post in posts {
            let tweet_id = Self::extract_tweet_id(&post.uri);
            
            if let Some(facets) = &post.record.facets {
                for facet in facets {
                    for feature in &facet.features {
                        match feature.type_field.as_str() {
                            "app.bsky.richtext.facet#tag" => {
                                if let Some(tag) = &feature.tag {
                                    all_hashtags.push((tweet_id.clone(), tag.clone(), facet.index.byte_start, facet.index.byte_end));
                                }
                            }
                            "app.bsky.richtext.facet#link" => {
                                if let Some(uri) = &feature.uri {
                                    all_urls.push((tweet_id.clone(), uri.clone(), facet.index.byte_start, facet.index.byte_end));
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }

            // Emojis depuis le texte
            let emojis = self.extract_emojis_from_text(&post.record.text);
            for (order, emoji) in emojis.iter().enumerate() {
                all_emojis.push((tweet_id.clone(), emoji.clone(), order as i32));
            }
        }

        const BATCH_SIZE: usize = 1000;
        self.bulk_insert_bluesky_hashtags(&all_hashtags, BATCH_SIZE).await?;
        self.bulk_insert_bluesky_urls(&all_urls, BATCH_SIZE).await?;
        self.bulk_insert_bluesky_emojis(&all_emojis, BATCH_SIZE).await?;

        tracing::info!("✅ Entités Bluesky insérées: {} hashtags, {} URLs, {} emojis", 
            all_hashtags.len(), all_urls.len(), all_emojis.len());

        // ÉTAPE 6: Médias
        self.bulk_insert_bluesky_media(posts).await?;

        let total_duration = start_time.elapsed();
        tracing::info!("🎉 Insertion ultra-scalable Bluesky terminée en {:?}", total_duration);

        Ok(posts.len())
    }

    // Helper pour extraire les emojis d'un texte
    fn extract_emojis_from_text(&self, text: &str) -> Vec<String> {
        let emoji_ranges = [
            '\u{1F600}'..='\u{1F64F}', // Emoticons
            '\u{1F300}'..='\u{1F5FF}', // Miscellaneous Symbols and Pictographs
            '\u{1F680}'..='\u{1F6FF}', // Transport and Map Symbols
            '\u{1F1E0}'..='\u{1F1FF}', // Regional Indicator Symbols (flags)
            '\u{2600}'..='\u{26FF}',   // Miscellaneous Symbols
            '\u{2700}'..='\u{27BF}',   // Dingbats
        ];
        
        let mut emojis = Vec::new();
        for ch in text.chars() {
            if emoji_ranges.iter().any(|range| range.contains(&ch)) {
                emojis.push(ch.to_string());
            }
        }
        emojis
    }

    // Helpers pour insertions massives Bluesky

    async fn bulk_insert_bluesky_users(&self, users: &[&Author]) -> Result<(), WebError> {
        if users.is_empty() { return Ok(()); }
        
        const BATCH_SIZE: usize = 1000;
        for chunk in users.chunks(BATCH_SIZE) {
            let ids: Vec<&str> = chunk.iter().map(|u| u.did.as_str()).collect();
            let screen_names: Vec<&str> = chunk.iter().map(|u| u.handle.as_str()).collect();
            let names: Vec<&str> = chunk.iter().map(|u| u.display_name.as_deref().unwrap_or("")).collect();
            let created_ats: Vec<Option<&str>> = chunk.iter().map(|_| None).collect(); // Bluesky Author n'a pas created_at
            // Bluesky n'a pas de champs verified/protected comme Twitter
            let verifieds: Vec<bool> = chunk.iter().map(|_| false).collect();
            let protecteds: Vec<bool> = chunk.iter().map(|_| false).collect();
            
            let query = format!(
                r#"INSERT INTO {}.user (id, screen_name, name, created_at, verified, protected)
                SELECT * FROM UNNEST($1::text[], $2::text[], $3::text[], $4::text[], $5::bool[], $6::bool[])
                ON CONFLICT (id) DO NOTHING"#, self.schema_name
            );
            
            sqlx::query(&query)
                .bind(ids).bind(screen_names).bind(names).bind(created_ats).bind(verifieds).bind(protecteds)
                .execute(&self.pool).await
                .map_err(|e| WebError::WTFError(format!("Bulk insert Bluesky users error: {}", e)))?;
        }
        Ok(())
    }

    async fn bulk_insert_bluesky_posts(&self, posts: &[Post]) -> Result<(), WebError> {
        if posts.is_empty() { return Ok(()); }
        
        const BATCH_SIZE: usize = 1000;
        for chunk in posts.chunks(BATCH_SIZE) {
            let ids: Vec<String> = chunk.iter().map(|p| Self::extract_tweet_id(&p.uri)).collect();
            let id_refs: Vec<&str> = ids.iter().map(|s| s.as_str()).collect();
            let created_ats: Vec<&str> = chunk.iter().map(|p| p.indexed_at.as_str()).collect();
            let published_times: Vec<i64> = chunk.iter().map(|p| {
                chrono::DateTime::parse_from_rfc3339(&p.indexed_at)
                    .unwrap_or_else(|_| chrono::Utc::now().into())
                    .timestamp_millis()  // CORRECTION: Millisecondes au lieu de secondes !
            }).collect();
            let user_ids: Vec<&str> = chunk.iter().map(|p| p.author.did.as_str()).collect();
            let user_names: Vec<&str> = chunk.iter().map(|p| {
                p.author.display_name.as_deref().unwrap_or("Unknown")
            }).collect();
            let user_screen_names: Vec<&str> = chunk.iter().map(|p| p.author.handle.as_str()).collect();
            let texts: Vec<&str> = chunk.iter().map(|p| p.record.text.as_str()).collect();
            
            // Bluesky n'a pas tous les champs de Twitter, on utilise des valeurs par défaut
            let sources: Vec<&str> = chunk.iter().map(|_| "Bluesky").collect();
            let languages: Vec<&str> = chunk.iter().map(|p| {
                // Essayer de détecter la langue depuis les langues déclarées
                if !p.record.langs.is_empty() {
                    p.record.langs.first().map(|s| s.as_str()).unwrap_or("unknown")
                } else {
                    "unknown"
                }
            }).collect();
            let possibly_sensitives: Vec<bool> = chunk.iter().map(|_| false).collect();
            
            // Métriques Bluesky
            let retweet_counts: Vec<i64> = chunk.iter().map(|p| p.repost_count as i64).collect();
            let reply_counts: Vec<i64> = chunk.iter().map(|p| p.reply_count as i64).collect();
            let quote_counts: Vec<i64> = chunk.iter().map(|p| p.quote_count as i64).collect();
            
            let query = format!(
                r#"INSERT INTO {}.tweet (id, created_at, published_time, user_id, user_name, user_screen_name, 
                   text, source, language, possibly_sensitive, retweet_count, reply_count, quote_count)
                SELECT * FROM UNNEST($1::text[], $2::text[], $3::bigint[], $4::text[], $5::text[], $6::text[],
                   $7::text[], $8::text[], $9::text[], $10::bool[], $11::bigint[], $12::bigint[], $13::bigint[])
                ON CONFLICT (id) DO NOTHING"#, self.schema_name
            );
            
            sqlx::query(&query)
                .bind(id_refs).bind(created_ats).bind(published_times).bind(user_ids)
                .bind(user_names).bind(user_screen_names).bind(texts).bind(sources)
                .bind(languages).bind(possibly_sensitives).bind(retweet_counts)
                .bind(reply_counts).bind(quote_counts)
                .execute(&self.pool).await
                .map_err(|e| WebError::WTFError(format!("Bulk insert Bluesky posts error: {}", e)))?;
        }
        Ok(())
    }

    async fn bulk_insert_bluesky_corpus(&self, corpus: &[(String, String)]) -> Result<(), WebError> {
        if corpus.is_empty() { return Ok(()); }
        
        const BATCH_SIZE: usize = 1000;
        for chunk in corpus.chunks(BATCH_SIZE) {
            let tweet_ids: Vec<&str> = chunk.iter().map(|(post_id, _)| post_id.as_str()).collect();
            let texts: Vec<&str> = chunk.iter().map(|(_, text)| text.as_str()).collect();
            
            let query = format!(
                r#"INSERT INTO {}.corpus (tweet_id, corpus)
                SELECT * FROM UNNEST($1::text[], $2::text[])
                ON CONFLICT (tweet_id) DO NOTHING"#, self.schema_name
            );
            
            sqlx::query(&query)
                .bind(tweet_ids).bind(texts)
                .execute(&self.pool).await
                .map_err(|e| WebError::WTFError(format!("Bulk insert Bluesky corpus error: {}", e)))?;
        }
        Ok(())
    }

    async fn bulk_insert_bluesky_replies(&self, replies: &[(String, String, String, String)]) -> Result<(), WebError> {
        if replies.is_empty() { return Ok(()); }
        
        const BATCH_SIZE: usize = 1000;
        for chunk in replies.chunks(BATCH_SIZE) {
            let tweet_ids: Vec<&str> = chunk.iter().map(|(post_id, _, _, _)| post_id.as_str()).collect();
            let reply_to_tweet_ids: Vec<&str> = chunk.iter().map(|(_, parent_id, _, _)| parent_id.as_str()).collect();
            let reply_to_user_ids: Vec<&str> = chunk.iter().map(|(_, _, user_id, _)| user_id.as_str()).collect();
            let screen_names: Vec<&str> = chunk.iter().map(|(_, _, _, screen_name)| screen_name.as_str()).collect();
            
            let query = format!(
                r#"INSERT INTO {}.reply (tweet_id, in_reply_to_tweet_id, in_reply_to_user_id, in_reply_to_screen_name)
                SELECT * FROM UNNEST($1::text[], $2::text[], $3::text[], $4::text[])
                ON CONFLICT (tweet_id) DO NOTHING"#, self.schema_name
            );
            
            sqlx::query(&query)
                .bind(tweet_ids).bind(reply_to_tweet_ids).bind(reply_to_user_ids).bind(screen_names)
                .execute(&self.pool).await
                .map_err(|e| WebError::WTFError(format!("Bulk insert Bluesky replies error: {}", e)))?;
        }
        Ok(())
    }

    async fn bulk_insert_bluesky_emojis(&self, emojis: &[(String, String, i32)], batch_size: usize) -> Result<(), WebError> {
        if emojis.is_empty() { return Ok(()); }
        
        for chunk in emojis.chunks(batch_size) {
            let tweet_ids: Vec<&str> = chunk.iter().map(|(post_id, _, _)| post_id.as_str()).collect();
            let emoji_chars: Vec<&str> = chunk.iter().map(|(_, emoji, _)| emoji.as_str()).collect();
            let orders: Vec<i32> = chunk.iter().map(|(_, _, order)| *order).collect();
            
            let query = format!(
                r#"INSERT INTO {}.tweet_emoji (tweet_id, emoji, "order")
                SELECT * FROM UNNEST($1::text[], $2::text[], $3::int[])
                ON CONFLICT (tweet_id, emoji) DO NOTHING"#, self.schema_name
            );
            
            sqlx::query(&query)
                .bind(tweet_ids).bind(emoji_chars).bind(orders)
                .execute(&self.pool).await
                .map_err(|e| WebError::WTFError(format!("Bulk insert Bluesky emojis error: {}", e)))?;
        }
        Ok(())
    }

    async fn bulk_insert_bluesky_hashtags(&self, hashtags: &[(String, String, i32, i32)], batch_size: usize) -> Result<(), WebError> {
        if hashtags.is_empty() { return Ok(()); }
        
        for chunk in hashtags.chunks(batch_size) {
            let tweet_ids: Vec<&str> = chunk.iter().map(|(tweet_id, _, _, _)| tweet_id.as_str()).collect();
            let hashtag_tags: Vec<&str> = chunk.iter().map(|(_, hashtag, _, _)| hashtag.as_str()).collect();
            let orders: Vec<i32> = chunk.iter().map(|(_, _, order, _)| *order).collect();
            let starts: Vec<i32> = chunk.iter().map(|(_, _, _, start)| *start).collect();
            // Bluesky n'a pas d'end indices dans les facets, on utilise start + longueur
            let ends: Vec<i32> = chunk.iter().map(|(_, hashtag, _, start)| *start + hashtag.len() as i32).collect();
            
            let query = format!(
                r#"INSERT INTO {}.tweet_hashtag (tweet_id, hashtag, "order", start_indice, end_indice)
                SELECT * FROM UNNEST($1::text[], $2::text[], $3::int[], $4::int[], $5::int[])
                ON CONFLICT (tweet_id, hashtag) DO NOTHING"#, self.schema_name
            );
            
            sqlx::query(&query)
                .bind(tweet_ids).bind(hashtag_tags).bind(orders).bind(starts).bind(ends)
                .execute(&self.pool).await
                .map_err(|e| WebError::WTFError(format!("Bulk insert Bluesky hashtags error: {}", e)))?;
        }
        Ok(())
    }

    async fn bulk_insert_bluesky_urls(&self, urls: &[(String, String, i32, i32)], batch_size: usize) -> Result<(), WebError> {
        if urls.is_empty() { return Ok(()); }
        
        for chunk in urls.chunks(batch_size) {
            let tweet_ids: Vec<&str> = chunk.iter().map(|(tweet_id, _, _, _)| tweet_id.as_str()).collect();
            let url_strings: Vec<&str> = chunk.iter().map(|(_, url, _, _)| url.as_str()).collect();
            let orders: Vec<i32> = chunk.iter().map(|(_, _, order, _)| *order).collect();
            let starts: Vec<i32> = chunk.iter().map(|(_, _, _, start)| *start).collect();
            // Bluesky n'a pas d'end indices dans les facets, on utilise start + longueur
            let ends: Vec<i32> = chunk.iter().map(|(_, url, _, start)| *start + url.len() as i32).collect();
            
            let query = format!(
                r#"INSERT INTO {}.tweet_url (tweet_id, url, "order", start_indice, end_indice)
                SELECT * FROM UNNEST($1::text[], $2::text[], $3::int[], $4::int[], $5::int[])
                ON CONFLICT (tweet_id, url) DO NOTHING"#, self.schema_name
            );
            
            sqlx::query(&query)
                .bind(tweet_ids).bind(url_strings).bind(orders).bind(starts).bind(ends)
                .execute(&self.pool).await
                .map_err(|e| WebError::WTFError(format!("Bulk insert Bluesky URLs error: {}", e)))?;
        }
        Ok(())
    }

    async fn bulk_insert_bluesky_media(&self, posts: &[Post]) -> Result<(), WebError> {
        let mut media_pairs = Vec::new();
        
        // Extraire les médias des posts Bluesky
        for post in posts {
            let tweet_id = Self::extract_tweet_id(&post.uri);
            
            if let Some(embed) = &post.embed {
                // Traiter les images
                if let Some(images) = embed.get("images") {
                    if let Some(images_array) = images.as_array() {
                        for (order, image) in images_array.iter().enumerate() {
                            if let Some(fullsize) = image.get("fullsize") {
                                if let Some(media_url) = fullsize.as_str() {
                                    media_pairs.push((tweet_id.clone(), media_url.to_string(), "image".to_string(), order as i32));
                                }
                            }
                        }
                    }
                }
                
                // Traiter les vidéos
                if let Some(video) = embed.get("video") {
                    if let Some(playlist) = video.get("playlist") {
                        if let Some(media_url) = playlist.as_str() {
                            media_pairs.push((tweet_id.clone(), media_url.to_string(), "video".to_string(), 0));
                        }
                    }
                }
                
                // Traiter les liens externes
                if let Some(external) = embed.get("external") {
                    if let Some(uri) = external.get("uri") {
                        if let Some(media_url) = uri.as_str() {
                            let media_type = if media_url.contains("youtube.com") || media_url.contains("youtu.be") {
                                "video"
                            } else if media_url.contains("instagram.com") || media_url.contains("tiktok.com") {
                                "social_media"
                            } else {
                                "link"
                            };
                            media_pairs.push((tweet_id.clone(), media_url.to_string(), media_type.to_string(), 0));
                        }
                    }
                }
            }
        }
        
        if media_pairs.is_empty() {
            return Ok(());
        }
        
        for chunk in media_pairs.chunks(1000) {
            let tweet_ids: Vec<&str> = chunk.iter().map(|(post_id, _, _, _)| post_id.as_str()).collect();
            let media_urls: Vec<&str> = chunk.iter().map(|(_, url, _, _)| url.as_str()).collect();
            let media_types: Vec<&str> = chunk.iter().map(|(_, _, type_field, _)| type_field.as_str()).collect();
            let orders: Vec<i32> = chunk.iter().map(|(_, _, _, order)| *order).collect();
            
            let query = format!(
                r#"INSERT INTO {}.tweet_media (tweet_id, media_url, type, "order")
                SELECT * FROM UNNEST($1::text[], $2::text[], $3::text[], $4::int[])
                ON CONFLICT (tweet_id, media_url) DO NOTHING"#, self.schema_name
            );
            
            sqlx::query(&query)
                .bind(tweet_ids).bind(media_urls).bind(media_types).bind(orders)
                .execute(&self.pool).await
                .map_err(|e| WebError::WTFError(format!("Bulk insert Bluesky media error: {}", e)))?;
        }
        Ok(())
    }
} 