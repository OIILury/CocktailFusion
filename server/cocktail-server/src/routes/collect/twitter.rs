use chrono::{DateTime, Utc};
use reqwest::header::{HeaderMap as ReqwestHeaderMap, HeaderValue, AUTHORIZATION};
use regex::Regex;

use crate::error::WebError;
use super::types::*;
use super::database::{TWITTER_API_URL_RECENT, TWITTER_API_URL_ALL};

pub struct TwitterCollector {
    client: reqwest::Client,
    pool: sqlx::PgPool,
    schema_name: String,
}

impl TwitterCollector {
    pub async fn new(bearer_token: &str, schema_name: String) -> Result<Self, WebError> {
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

    pub async fn search_tweets(&self, keyword: &str, limit: usize, start_date: Option<&str>, end_date: Option<&str>) -> Result<TwitterSearchResponse, WebError> {
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

    pub async fn save_tweet_to_db(&self, tweet: &TwitterTweet, includes: Option<&TwitterIncludes>) -> Result<(), WebError> {
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