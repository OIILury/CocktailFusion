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

    // Nouvelle méthode optimisée pour traitement en batch des posts Bluesky
    pub async fn save_posts_batch_to_db(&self, posts: &[Post]) -> Result<usize, WebError> {
        let mut saved_count = 0;
        let batch_size = 50; // Traiter par batches de 50 posts pour optimiser
        
        for batch in posts.chunks(batch_size) {
            // Utiliser une transaction pour le batch
            let mut tx = self.pool.begin().await
                .map_err(|e| WebError::WTFError(format!("Failed to start transaction: {}", e)))?;
            
            for post in batch {
                if let Err(e) = self.save_single_post_in_transaction(&mut tx, post).await {
                    tracing::warn!("Error saving Bluesky post {} in batch: {}", post.uri, e);
                } else {
                    saved_count += 1;
                }
            }
            
            // Commit du batch
            tx.commit().await
                .map_err(|e| WebError::WTFError(format!("Failed to commit batch: {}", e)))?;
                
            if batch.len() == batch_size {
                tracing::debug!("Batch of {} posts processed, total saved: {}", batch_size, saved_count);
            }
        }
        
        tracing::info!("Batch processing completed: {}/{} posts saved", saved_count, posts.len());
        Ok(saved_count)
    }

    // Version modifiée pour travailler avec une transaction
    async fn save_single_post_in_transaction(&self, tx: &mut sqlx::Transaction<'_, sqlx::Postgres>, post: &Post) -> Result<(), WebError> {
        let created_at = DateTime::parse_from_rfc3339(&post.indexed_at)
            .map_err(|e| WebError::WTFError(format!("Date parse error: {}", e)))?
            .with_timezone(&chrono::Utc);
            
        let default_lang = "fr".to_string();
        let lang = post.record.langs.first().unwrap_or(&default_lang);
        let tweet_id = Self::extract_tweet_id(&post.uri);
        
        // Insérer le tweet principal avec transaction
        self.insert_basic_tweet_with_tx(tx, &tweet_id, created_at, &post.author, &post.record.text, "Bluesky", lang, post.repost_count, post.reply_count, post.quote_count).await?;
        
        // Pour l'instant, on se concentre sur l'insertion de base avec transactions
        // Les autres optimisations (hashtags, médias, etc.) peuvent être ajoutées progressivement
        
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
        .execute(&mut **tx)
        .await
        .map_err(|e| WebError::WTFError(format!("DB insert corpus error: {}", e)))?;
        
        Ok(())
    }

    // Version avec transaction pour l'insertion de base
    async fn insert_basic_tweet_with_tx(&self, tx: &mut sqlx::Transaction<'_, sqlx::Postgres>, tweet_id: &str, created_at: DateTime<chrono::Utc>, author: &Author, 
                               text: &str, source: &str, lang: &str, retweet_count: i64, 
                               reply_count: i64, quote_count: i64) -> Result<(), WebError> {
        // D'abord insérer l'utilisateur avec transaction
        self.insert_user_with_tx(tx, author, created_at).await?;
        
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
        .execute(&mut **tx)
        .await
        .map_err(|e| WebError::WTFError(format!("DB insert tweet error: {}", e)))?;
        
        Ok(())
    }

    // Version avec transaction pour l'insertion d'utilisateur
    async fn insert_user_with_tx(&self, tx: &mut sqlx::Transaction<'_, sqlx::Postgres>, author: &Author, created_at: DateTime<chrono::Utc>) -> Result<(), WebError> {
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
        .execute(&mut **tx)
        .await
        .map_err(|e| WebError::WTFError(format!("DB insert user error: {}", e)))?;
        
        Ok(())
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
} 