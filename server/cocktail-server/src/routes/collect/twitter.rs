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
        tracing::info!("Starting optimized Twitter search for keyword: '{}', limit: {}", keyword, limit);
        
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
        
        // Optimisations pour gros volumes :
        // - Augmenter drastiquement le nombre de requêtes max (pour 100k+ tweets)
        // - Calculer dynamiquement basé sur la limite demandée
        // - LIMITATION: context_annotations limite max_results à 100 par requête
        let max_requests = if limit > 10000 {
            (limit / 100) + 100 // Pour 100k tweets avec limitation 100/requête : 1000 + 100 = 1100 requêtes max
        } else {
            (limit / 100) + 20 // Pour volumes plus petits avec limitation 100/requête
        };
        
        tracing::info!("Configured for max {} requests to collect {} tweets", max_requests, limit);
        
        while tweets.len() < limit && request_count < max_requests {
            // OPTIMISATION GROS VOLUMES : Utiliser le maximum API Twitter (100 tweets par requête avec context_annotations)
            let remaining_tweets = limit - tweets.len();
            let batch_limit = if limit > 10000 {
                // Pour gros volumes : toujours demander le maximum (100 à cause de context_annotations)
                100
            } else {
                // Pour petits volumes : ajuster au besoin
                remaining_tweets.min(100)
            };
            
            // Twitter API requires max_results to be between 10 and 500
            // MAIS: quand context_annotations est demandé, max_results doit être <= 100
            let twitter_max_results = batch_limit.max(10).min(100); // Limité à 100 à cause de context_annotations
            
            // TOUS LES CHAMPS pour une collecte complète (même pour gros volumes)
            let mut params = vec![
                ("query", keyword.to_string()),
                ("max_results", twitter_max_results.to_string()),
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
            
                         // Logs détaillés pour gros volumes
             if limit > 10000 {
                 let progress_pct = (tweets.len() as f64 / limit as f64 * 100.0).round() as u32;
                 
                 // Calculer le délai avant de l'utiliser dans l'estimation
                 let default_delay = if limit > 100000 { 1 } else if limit > 10000 { 2 } else { 4 };
                 let delay_seconds = std::env::var("TWITTER_RATE_LIMIT_DELAY_SECONDS")
                     .ok()
                     .and_then(|s| s.parse::<u64>().ok())
                     .unwrap_or(default_delay);
                 
                 let est_time_remaining = if tweets.len() > 0 {
                     let tweets_per_request = tweets.len() as f64 / request_count as f64;
                     let remaining_requests = ((limit - tweets.len()) as f64 / tweets_per_request).ceil() as u64;
                     let time_per_request = delay_seconds + 2; // délai + temps requête estimé
                     remaining_requests * time_per_request
                 } else { 0 };
                
                tracing::info!("🚀 GROS VOLUME #{}/{} - Lot: {} tweets, Progrès: {}/{} ({}%) - ETA: {}min", 
                    request_count + 1, max_requests, twitter_max_results, tweets.len(), limit, progress_pct, est_time_remaining / 60);
            } else {
                tracing::info!("Twitter API request #{}/{} - Demandé: {} tweets, Collecté: {}/{} tweets", 
                    request_count + 1, max_requests, twitter_max_results, tweets.len(), limit);
            }
            
            let start_time = std::time::Instant::now();
            let response = self.client
                .get(api_url)
                .query(&params)
                .send()
                .await
                .map_err(|e| WebError::WTFError(format!("Twitter API request error: {}", e)))?;
            
            let request_duration = start_time.elapsed();
            if limit <= 10000 {
                tracing::info!("Requête API terminée en {:?}", request_duration);
            }
            
            if !response.status().is_success() {
                let status = response.status();
                let error_text = response.text().await.unwrap_or_default();
                tracing::error!("Twitter API error {}: {}", status, error_text);
                
                // Gestion spéciale des erreurs 429 (Too Many Requests)
                if status == 429 {
                    tracing::warn!("Rate limit atteint, attente de 15 minutes avant de continuer...");
                    tokio::time::sleep(tokio::time::Duration::from_secs(15 * 60)).await; // 15 minutes
                    tracing::info!("Fin d'attente, reprise de la collecte...");
                    continue; // Retry la même requête après le délai
                }
                
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
            if limit <= 10000 {
                tracing::info!("Parsing JSON terminé en {:?}", parse_duration);
            }
            
            if let Some(data) = search_response.data {
                if data.is_empty() {
                    tracing::info!("No more tweets found, stopping search");
                    break;
                }
                
                // Ne prendre que le nombre de tweets nécessaires pour ne pas dépasser la limite
                let remaining_slots = limit - tweets.len();
                let tweets_to_add = data.into_iter().take(remaining_slots).collect::<Vec<_>>();
                let new_tweets_count = tweets_to_add.len();
                
                tweets.extend(tweets_to_add);
                if limit <= 10000 || request_count % 50 == 0 {
                    tracing::info!("Ajouté {} nouveaux tweets (total: {}/{})", new_tweets_count, tweets.len(), limit);
                }
                
                // Si on a atteint la limite demandée, arrêter la collecte
                if tweets.len() >= limit {
                    tracing::info!("Reached requested limit of {} tweets, stopping search", limit);
                    break;
                }
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
            
            // OPTIMISATION RATE LIMITS - Plus agressif pour gros volumes
            let default_delay = if limit > 100000 {
                0 // 0 seconde pour très gros volumes (100k+) - le plus rapide possible
            } else if limit > 10000 {
                0 // 0 seconde pour gros volumes (10k+) - rapide
            } else {
                1 // 1 seconde pour volumes normaux - compromis
            };
            
            let delay_seconds = std::env::var("TWITTER_RATE_LIMIT_DELAY_SECONDS")
                .ok()
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(default_delay);
            
            let sleep_duration = tokio::time::Duration::from_secs(delay_seconds);
            
            if delay_seconds > 0 {
                if limit <= 10000 {
                    tracing::debug!("Attente de {:?} avant la prochaine requête (respect des rate limits)", sleep_duration);
                }
                tokio::time::sleep(sleep_duration).await;
            }
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

    // Helper pour insérer des hashtags depuis l'API Twitter - VERSION BATCH
    async fn insert_twitter_hashtags(&self, tweet_id: &str, hashtags: &[TwitterHashtag]) -> Result<(), WebError> {
        if hashtags.is_empty() {
            return Ok(());
        }
        
        // Traiter par lots de 100 pour éviter les arrays trop volumineux
        const BATCH_SIZE: usize = 100;
        for chunk in hashtags.chunks(BATCH_SIZE) {
            // Préparer les données en arrays
            let tweet_ids: Vec<&str> = chunk.iter().map(|_| tweet_id).collect();
            let tags: Vec<&str> = chunk.iter().map(|h| h.tag.as_str()).collect();
            let orders: Vec<i32> = (0..chunk.len()).map(|i| i as i32).collect();
            let starts: Vec<i32> = chunk.iter().map(|h| h.start).collect();
            let ends: Vec<i32> = chunk.iter().map(|h| h.end).collect();
            
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
                .map_err(|e| WebError::WTFError(format!("DB batch insert Twitter hashtags error: {}", e)))?;
        }
        
        Ok(())
    }

    // Helper pour insérer des URLs depuis l'API Twitter - VERSION BATCH
    async fn insert_twitter_urls(&self, tweet_id: &str, urls: &[TwitterUrl]) -> Result<(), WebError> {
        if urls.is_empty() {
            return Ok(());
        }
        
        const BATCH_SIZE: usize = 100;
        for chunk in urls.chunks(BATCH_SIZE) {
            let tweet_ids: Vec<&str> = chunk.iter().map(|_| tweet_id).collect();
            let final_urls: Vec<String> = chunk.iter()
                .map(|url| url.expanded_url.as_ref().unwrap_or(&url.url).clone())
                .collect();
            let url_refs: Vec<&str> = final_urls.iter().map(|s| s.as_str()).collect();
            let orders: Vec<i32> = (0..chunk.len()).map(|i| i as i32).collect();
            let starts: Vec<i32> = chunk.iter().map(|u| u.start).collect();
            let ends: Vec<i32> = chunk.iter().map(|u| u.end).collect();
            
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
                .bind(url_refs)
                .bind(orders)
                .bind(starts)
                .bind(ends)
                .execute(&self.pool)
                .await
                .map_err(|e| WebError::WTFError(format!("DB batch insert Twitter URLs error: {}", e)))?;
        }
        
        Ok(())
    }

    // Helper pour insérer des cashtags depuis l'API Twitter - VERSION BATCH
    async fn insert_twitter_cashtags(&self, tweet_id: &str, cashtags: &[TwitterCashtag]) -> Result<(), WebError> {
        if cashtags.is_empty() {
            return Ok(());
        }
        
        const BATCH_SIZE: usize = 100;
        for chunk in cashtags.chunks(BATCH_SIZE) {
            let tweet_ids: Vec<&str> = chunk.iter().map(|_| tweet_id).collect();
            let cashtags_with_symbol: Vec<String> = chunk.iter()
                .map(|c| format!("${}", c.tag))
                .collect();
            let cashtag_refs: Vec<&str> = cashtags_with_symbol.iter().map(|s| s.as_str()).collect();
            let orders: Vec<i32> = (0..chunk.len()).map(|i| i as i32).collect();
            let starts: Vec<i32> = chunk.iter().map(|c| c.start).collect();
            let ends: Vec<i32> = chunk.iter().map(|c| c.end).collect();
            
            let query = format!(
                r#"
                INSERT INTO {}.tweet_cashtag (tweet_id, cashtag, "order", start_indice, end_indice)
                SELECT * FROM UNNEST($1::text[], $2::text[], $3::int[], $4::int[], $5::int[])
                ON CONFLICT (tweet_id, cashtag) DO NOTHING
                "#,
                self.schema_name
            );
            
            sqlx::query(&query)
                .bind(tweet_ids)
                .bind(cashtag_refs)
                .bind(orders)
                .bind(starts)
                .bind(ends)
                .execute(&self.pool)
                .await
                .map_err(|e| WebError::WTFError(format!("DB batch insert Twitter cashtags error: {}", e)))?;
        }
        
        Ok(())
    }

    // Helper pour insérer des mentions d'utilisateurs depuis l'API Twitter - VERSION BATCH
    async fn insert_twitter_mentions(&self, tweet_id: &str, mentions: &[TwitterMention]) -> Result<(), WebError> {
        if mentions.is_empty() {
            return Ok(());
        }
        
        const BATCH_SIZE: usize = 100;
        for chunk in mentions.chunks(BATCH_SIZE) {
            let tweet_ids: Vec<&str> = chunk.iter().map(|_| tweet_id).collect();
            let user_ids: Vec<&str> = chunk.iter()
                .map(|m| m.id.as_ref().unwrap_or(&m.username).as_str())
                .collect();
            let orders: Vec<i32> = (0..chunk.len()).map(|i| i as i32).collect();
            let starts: Vec<i32> = chunk.iter().map(|m| m.start).collect();
            let ends: Vec<i32> = chunk.iter().map(|m| m.end).collect();
            
            let query = format!(
                r#"
                INSERT INTO {}.tweet_user_mention (tweet_id, user_id, "order", start_indice, end_indice)
                SELECT * FROM UNNEST($1::text[], $2::text[], $3::int[], $4::int[], $5::int[])
                ON CONFLICT (tweet_id, user_id) DO NOTHING
                "#,
                self.schema_name
            );
            
            sqlx::query(&query)
                .bind(tweet_ids)
                .bind(user_ids)
                .bind(orders)
                .bind(starts)
                .bind(ends)
                .execute(&self.pool)
                .await
                .map_err(|e| WebError::WTFError(format!("DB batch insert Twitter mentions error: {}", e)))?;
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

    // Nouvelle méthode optimisée pour traitement en batch
    pub async fn save_tweets_batch_to_db(&self, tweets: &[TwitterTweet], includes: Option<&TwitterIncludes>) -> Result<usize, WebError> {
        let mut saved_count = 0;
        
        // Traitement séquentiel mais fiable - évite les deadlocks de transaction
        for tweet in tweets {
            if let Err(e) = self.save_tweet_to_db(tweet, includes).await {
                tracing::warn!("Error saving tweet {} in batch: {}", tweet.id, e);
            } else {
                saved_count += 1;
            }
            
            // Log de progression pour voir l'avancement
            if saved_count % 10 == 0 {
                tracing::info!("Saved {}/{} tweets...", saved_count, tweets.len());
            }
        }
        
        tracing::info!("Batch processing completed: {}/{} tweets saved", saved_count, tweets.len());
        Ok(saved_count)
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

    // Nouvelle méthode pour insérer TOUTES les données de TOUS les tweets en une fois - ULTRA SCALABLE COMPLET
    pub async fn save_all_tweets_ultra_batch(&self, tweets: &[TwitterTweet], includes: Option<&TwitterIncludes>) -> Result<usize, WebError> {
        if tweets.is_empty() {
            return Ok(0);
        }

        tracing::info!("🚀 Démarrage insertion ultra-scalable pour {} tweets", tweets.len());
        let start_time = std::time::Instant::now();

        // 1. ÉTAPE 1: Collecter et insérer tous les utilisateurs en masse
        let mut all_users = Vec::new();
        
        // Users depuis les includes
        if let Some(includes) = includes {
            if let Some(users) = &includes.users {
                all_users.extend(users.iter());
            }
        }
        
        // Users fantômes pour TOUS les utilisateurs manquants (auteurs + mentionnés + réponses)
        let mut phantom_users = Vec::new();
        
        // Vérifier les auteurs des tweets principaux
        for tweet in tweets {
            if !all_users.iter().any(|u| u.id == tweet.author_id) {
                phantom_users.push(&tweet.author_id);
            }
        }
        
        // Vérifier les auteurs des tweets référencés
        if let Some(includes) = includes {
            if let Some(ref_tweets) = &includes.tweets {
                for ref_tweet in ref_tweets {
                    if !all_users.iter().any(|u| u.id == ref_tweet.author_id) && 
                       !phantom_users.contains(&&ref_tweet.author_id) {
                        phantom_users.push(&ref_tweet.author_id);
                    }
                }
            }
        }
        
        // CRITIQUE: Collecter TOUS les users mentionnés pour éviter les violations FK
        for tweet in tweets {
            // Users mentionnés dans les entités
            if let Some(entities) = &tweet.entities {
                if let Some(mentions) = &entities.mentions {
                    for mention in mentions {
                        let user_id = mention.id.as_ref().unwrap_or(&mention.username);
                        if !all_users.iter().any(|u| u.id == *user_id) && 
                           !phantom_users.contains(&user_id) {
                            phantom_users.push(user_id);
                        }
                    }
                }
            }
            
            // Users mentionnés dans les réponses (in_reply_to_user_id)
            if let Some(in_reply_to_user_id) = &tweet.in_reply_to_user_id {
                if !all_users.iter().any(|u| u.id == *in_reply_to_user_id) && 
                   !phantom_users.contains(&in_reply_to_user_id) {
                    phantom_users.push(in_reply_to_user_id);
                }
            }
        }
        
        // Insérer tous les users en batch
        self.bulk_insert_users(&all_users).await?;
        self.bulk_insert_phantom_users(&phantom_users).await?;
        tracing::info!("✅ Users insérés: {} réels + {} fantômes", all_users.len(), phantom_users.len());
        tracing::debug!("🔍 IDs fantômes créés: {:?}", phantom_users.iter().take(10).collect::<Vec<_>>());

        // 2. ÉTAPE 2: Collecter et insérer tous les lieux en masse
        if let Some(includes) = includes {
            if let Some(places) = &includes.places {
                self.bulk_insert_places(places).await?;
                tracing::info!("✅ Places insérées: {}", places.len());
            }
        }

        // 3. ÉTAPE 3: PRIORITÉ ABSOLUE - Insérer TOUS les tweets d'abord (table de base)
        let mut all_tweets_to_insert = Vec::new();
        
        // Ajouter tous les tweets référencés
        if let Some(includes) = includes {
            if let Some(ref_tweets) = &includes.tweets {
                all_tweets_to_insert.extend(ref_tweets.iter());
            }
        }
        
        // Ajouter tous les tweets principaux
        all_tweets_to_insert.extend(tweets.iter());
        
        // CRITIQUE: Insérer tous les tweets AVANT toute autre relation
        self.bulk_insert_tweets(&all_tweets_to_insert, &all_users).await?;
        tracing::info!("🎯 PRIORITÉ: Tous les tweets insérés en premier: {} (base pour toutes les FK)", all_tweets_to_insert.len());

        // 4. ÉTAPE 4: Collecter tous les IDs de tweets disponibles pour validation des FK
        let mut all_available_tweet_ids = std::collections::HashSet::new();
        
        // Ajouter les IDs des tweets principaux
        for tweet in tweets {
            all_available_tweet_ids.insert(tweet.id.as_str());
        }
        
        // Ajouter les IDs des tweets référencés
        if let Some(includes) = includes {
            if let Some(ref_tweets) = &includes.tweets {
                for ref_tweet in ref_tweets {
                    all_available_tweet_ids.insert(ref_tweet.id.as_str());
                }
            }
        }

        // 5. ÉTAPE 5: Collecter et insérer toutes les relations en masse avec validation FK
        let mut all_replies = Vec::new();
        let mut all_retweets = Vec::new();
        let mut all_quotes = Vec::new();
        let mut all_tweet_places = Vec::new();
        let mut all_corpus = Vec::new();
        let mut all_withheld = Vec::new();

        for tweet in tweets {
            // Relations replies/retweets/quotes avec validation FK
            if let Some(in_reply_to_user_id) = &tweet.in_reply_to_user_id {
                if let Some(referenced_tweets) = &tweet.referenced_tweets {
                    for ref_tweet in referenced_tweets {
                        // Vérifier que le tweet référencé existe dans notre jeu de données
                        if all_available_tweet_ids.contains(ref_tweet.id.as_str()) {
                            match ref_tweet.ref_type.as_str() {
                                "replied_to" => {
                                    all_replies.push((&tweet.id, &ref_tweet.id, in_reply_to_user_id, "unknown"));
                                },
                                "retweeted" => {
                                    all_retweets.push((&tweet.id, &ref_tweet.id));
                                },
                                "quoted" => {
                                    all_quotes.push((&tweet.id, &ref_tweet.id));
                                },
                                _ => {}
                            }
                        } else {
                            tracing::warn!("Tweet référencé {} non trouvé dans le jeu de données, relation ignorée", ref_tweet.id);
                        }
                    }
                }
            } else if let Some(referenced_tweets) = &tweet.referenced_tweets {
                for ref_tweet in referenced_tweets {
                    // Vérifier que le tweet référencé existe dans notre jeu de données
                    if all_available_tweet_ids.contains(ref_tweet.id.as_str()) {
                        match ref_tweet.ref_type.as_str() {
                            "retweeted" => {
                                all_retweets.push((&tweet.id, &ref_tweet.id));
                            },
                            "quoted" => {
                                all_quotes.push((&tweet.id, &ref_tweet.id));
                            },
                            _ => {}
                        }
                    } else {
                        tracing::warn!("Tweet référencé {} non trouvé dans le jeu de données, relation ignorée", ref_tweet.id);
                    }
                }
            }

            // Places
            if let Some(geo) = &tweet.geo {
                if let Some(place_id) = &geo.place_id {
                    all_tweet_places.push((&tweet.id, place_id));
                }
            }

            // Corpus
            all_corpus.push((&tweet.id, &tweet.text));

            // Withheld countries
            if let Some(withheld) = &tweet.withheld {
                if let Some(country_codes) = &withheld.country_codes {
                    for country in country_codes {
                        all_withheld.push((&tweet.author_id, country));
                    }
                }
            }
        }

        // Insérer toutes les relations en batch
        // Convertir les types String vers &str
        let all_replies_str: Vec<(&str, &str, &str, &str)> = all_replies.iter()
            .map(|(a, b, c, d)| (a.as_str(), b.as_str(), c.as_str(), *d)).collect();
        let all_retweets_str: Vec<(&str, &str)> = all_retweets.iter()
            .map(|(a, b)| (a.as_str(), b.as_str())).collect();
        let all_quotes_str: Vec<(&str, &str)> = all_quotes.iter()
            .map(|(a, b)| (a.as_str(), b.as_str())).collect();
        let all_tweet_places_str: Vec<(&str, &str)> = all_tweet_places.iter()
            .map(|(a, b)| (a.as_str(), b.as_str())).collect();
        let all_corpus_str: Vec<(&str, &str)> = all_corpus.iter()
            .map(|(a, b)| (a.as_str(), b.as_str())).collect();
        let all_withheld_str: Vec<(&str, &str)> = all_withheld.iter()
            .map(|(a, b)| (a.as_str(), b.as_str())).collect();

        self.bulk_insert_replies(&all_replies_str).await?;
        self.bulk_insert_retweets(&all_retweets_str).await?;
        self.bulk_insert_quotes(&all_quotes_str).await?;
        self.bulk_insert_tweet_places(&all_tweet_places_str).await?;
        self.bulk_insert_corpus(&all_corpus_str).await?;
        self.bulk_insert_withheld(&all_withheld_str).await?;
        
        tracing::info!("✅ Relations insérées: {} replies, {} RTs, {} quotes, {} places, {} corpus, {} withheld", 
            all_replies.len(), all_retweets.len(), all_quotes.len(), all_tweet_places.len(), all_corpus.len(), all_withheld.len());

        // 6. ÉTAPE 6: Collecter et insérer toutes les entités en masse
        let mut all_hashtags = Vec::new();
        let mut all_urls = Vec::new();
        let mut all_cashtags = Vec::new();
        let mut all_mentions = Vec::new();
        let mut all_emojis = Vec::new();

        for tweet in tweets {
            if let Some(entities) = &tweet.entities {
                // Hashtags
                if let Some(hashtags) = &entities.hashtags {
                    for (order, hashtag) in hashtags.iter().enumerate() {
                        all_hashtags.push((&tweet.id, &hashtag.tag, order as i32, hashtag.start, hashtag.end));
                    }
                }

                // URLs
                if let Some(urls) = &entities.urls {
                    for (order, url) in urls.iter().enumerate() {
                        let final_url = url.expanded_url.as_ref().unwrap_or(&url.url);
                        all_urls.push((&tweet.id, final_url.as_str(), order as i32, url.start, url.end));
                    }
                }

                // Cashtags
                if let Some(cashtags) = &entities.cashtags {
                    for (order, cashtag) in cashtags.iter().enumerate() {
                        all_cashtags.push((&tweet.id, format!("${}", cashtag.tag), order as i32, cashtag.start, cashtag.end));
                    }
                }

                // Mentions
                if let Some(mentions) = &entities.mentions {
                    for (order, mention) in mentions.iter().enumerate() {
                        let user_id = mention.id.as_ref().unwrap_or(&mention.username);
                        all_mentions.push((&tweet.id, user_id.as_str(), order as i32, mention.start, mention.end));
                    }
                }
            }

            // Emojis depuis le texte
            let emojis = self.extract_emojis_from_text(&tweet.text);
            for (order, emoji) in emojis.iter().enumerate() {
                all_emojis.push((tweet.id.clone(), emoji.clone(), order as i32));
            }
        }

        // Convertir les types vers les bonnes signatures
        let all_hashtags_converted: Vec<(&str, &str, i32, usize, usize)> = all_hashtags.iter()
            .map(|(a, b, c, d, e)| (a.as_str(), b.as_str(), *c, *d as usize, *e as usize)).collect();
        let all_urls_converted: Vec<(&str, &str, i32, usize, usize)> = all_urls.iter()
            .map(|(a, b, c, d, e)| (a.as_str(), *b, *c, *d as usize, *e as usize)).collect();
        let all_cashtags_converted: Vec<(&str, String, i32, usize, usize)> = all_cashtags.iter()
            .map(|(a, b, c, d, e)| (a.as_str(), b.clone(), *c, *d as usize, *e as usize)).collect();
        let all_mentions_converted: Vec<(&str, &str, i32, usize, usize)> = all_mentions.iter()
            .map(|(a, b, c, d, e)| (a.as_str(), *b, *c, *d as usize, *e as usize)).collect();

        // Insérer toutes les entités en batch
        const BATCH_SIZE: usize = 1000;
        self.bulk_insert_hashtags(&all_hashtags_converted, BATCH_SIZE).await?;
        self.bulk_insert_urls(&all_urls_converted, BATCH_SIZE).await?;
        self.bulk_insert_cashtags(&all_cashtags_converted, BATCH_SIZE).await?;
        
        // Debug des mentions avant insertion
        if !all_mentions_converted.is_empty() {
            tracing::debug!("🔍 Tentative d'insertion de {} mentions", all_mentions_converted.len());
            tracing::debug!("🔍 Premières mentions: {:?}", all_mentions_converted.iter().take(5).collect::<Vec<_>>());
        }
        self.bulk_insert_mentions(&all_mentions_converted, BATCH_SIZE).await?;
        // Convertir les types String vers &str pour emojis
        let all_emojis_str: Vec<(&str, &str, i32)> = all_emojis.iter()
            .map(|(a, b, c)| (a.as_str(), b.as_str(), *c)).collect();
        self.bulk_insert_emojis(&all_emojis_str).await?;

        tracing::info!("✅ Entités insérées: {} hashtags, {} URLs, {} cashtags, {} mentions, {} emojis", 
            all_hashtags.len(), all_urls.len(), all_cashtags.len(), all_mentions.len(), all_emojis.len());

        // 7. ÉTAPE 7: Insérer les médias en batch
        if let Some(includes) = includes {
            if let Some(media_list) = &includes.media {
                self.bulk_insert_media(tweets, media_list).await?;
                tracing::info!("✅ Médias insérés: {}", media_list.len());
            }
        }

        let total_duration = start_time.elapsed();
        tracing::info!("🎉 Insertion ultra-scalable terminée en {:?} pour {} tweets", total_duration, tweets.len());

        Ok(tweets.len())
    }

    // Helper pour extraire les emojis d'un texte
    fn extract_emojis_from_text(&self, text: &str) -> Vec<String> {
        // Regex simplifiée pour les emojis (caractères Unicode dans les plages emoji)
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

    // Helpers pour insertions massives de TOUTES les tables

    async fn bulk_insert_users(&self, users: &[&TwitterUser]) -> Result<(), WebError> {
        if users.is_empty() { return Ok(()); }
        
        const BATCH_SIZE: usize = 1000;
        for chunk in users.chunks(BATCH_SIZE) {
            let ids: Vec<&str> = chunk.iter().map(|u| u.id.as_str()).collect();
            let screen_names: Vec<&str> = chunk.iter().map(|u| u.username.as_str()).collect();
            let names: Vec<Option<&str>> = chunk.iter().map(|u| Some(u.name.as_str())).collect();
            let created_ats: Vec<Option<&str>> = chunk.iter().map(|u| u.created_at.as_deref()).collect();
            let verifieds: Vec<bool> = chunk.iter().map(|u| u.verified.unwrap_or(false)).collect();
            let protecteds: Vec<bool> = chunk.iter().map(|u| u.protected.unwrap_or(false)).collect();
            
            let query = format!(
                r#"INSERT INTO {}.user (id, screen_name, name, created_at, verified, protected)
                SELECT * FROM UNNEST($1::text[], $2::text[], $3::text[], $4::text[], $5::bool[], $6::bool[])
                ON CONFLICT (id) DO NOTHING"#, self.schema_name
            );
            
            sqlx::query(&query)
                .bind(ids).bind(screen_names).bind(names).bind(created_ats).bind(verifieds).bind(protecteds)
                .execute(&self.pool).await
                .map_err(|e| WebError::WTFError(format!("Bulk insert users error: {}", e)))?;
        }
        Ok(())
    }

    async fn bulk_insert_phantom_users(&self, user_ids: &[&String]) -> Result<(), WebError> {
        if user_ids.is_empty() { return Ok(()); }
        
        const BATCH_SIZE: usize = 1000;
        for chunk in user_ids.chunks(BATCH_SIZE) {
            let ids: Vec<&str> = chunk.iter().map(|id| id.as_str()).collect();
            let screen_names: Vec<String> = chunk.iter().map(|id| format!("unknown_{}", id)).collect();
            let screen_name_refs: Vec<&str> = screen_names.iter().map(|s| s.as_str()).collect();
            
            let query = format!(
                r#"INSERT INTO {}.user (id, screen_name, name, verified, protected)
                SELECT * FROM UNNEST($1::text[], $2::text[], $3::text[], $4::bool[], $5::bool[])
                ON CONFLICT (id) DO NOTHING"#, self.schema_name
            );
            
            let names: Vec<&str> = chunk.iter().map(|_| "Unknown User").collect();
            let verifieds: Vec<bool> = chunk.iter().map(|_| false).collect();
            let protecteds: Vec<bool> = chunk.iter().map(|_| false).collect();
            
            sqlx::query(&query)
                .bind(ids).bind(screen_name_refs).bind(names).bind(verifieds).bind(protecteds)
                .execute(&self.pool).await
                .map_err(|e| WebError::WTFError(format!("Bulk insert phantom users error: {}", e)))?;
        }
        Ok(())
    }

    async fn bulk_insert_places(&self, places: &[TwitterPlace]) -> Result<(), WebError> {
        if places.is_empty() { return Ok(()); }
        
        const BATCH_SIZE: usize = 1000;
        for chunk in places.chunks(BATCH_SIZE) {
            let ids: Vec<&str> = chunk.iter().map(|p| p.id.as_str()).collect();
            let names: Vec<Option<&str>> = chunk.iter().map(|p| Some(p.name.as_str())).collect();
            let full_names: Vec<Option<&str>> = chunk.iter().map(|p| Some(p.full_name.as_str())).collect();
            let country_codes: Vec<Option<&str>> = chunk.iter().map(|p| p.country_code.as_deref()).collect();
            let countries: Vec<Option<&str>> = chunk.iter().map(|p| p.country.as_deref()).collect();
            let place_types: Vec<Option<&str>> = chunk.iter().map(|p| p.place_type.as_deref()).collect();
            
            let query = format!(
                r#"INSERT INTO {}.place (id, name, full_name, country_code, country, place_type)
                SELECT * FROM UNNEST($1::text[], $2::text[], $3::text[], $4::text[], $5::text[], $6::text[])
                ON CONFLICT (id) DO NOTHING"#, self.schema_name
            );
            
            sqlx::query(&query)
                .bind(ids).bind(names).bind(full_names).bind(country_codes).bind(countries).bind(place_types)
                .execute(&self.pool).await
                .map_err(|e| WebError::WTFError(format!("Bulk insert places error: {}", e)))?;
        }
        Ok(())
    }

    async fn bulk_insert_tweets(&self, tweets: &[&TwitterTweet], users: &[&TwitterUser]) -> Result<(), WebError> {
        if tweets.is_empty() { return Ok(()); }
        
        const BATCH_SIZE: usize = 1000;
        for chunk in tweets.chunks(BATCH_SIZE) {
            let ids: Vec<&str> = chunk.iter().map(|t| t.id.as_str()).collect();
            let created_ats: Vec<&str> = chunk.iter().map(|t| t.created_at.as_str()).collect();
            let published_times: Vec<i64> = chunk.iter().map(|t| {
                chrono::DateTime::parse_from_rfc3339(&t.created_at)
                    .unwrap_or_else(|_| chrono::Utc::now().into())
                    .timestamp_millis()  // CORRECTION: Millisecondes au lieu de secondes !
            }).collect();
            let user_ids: Vec<&str> = chunk.iter().map(|t| t.author_id.as_str()).collect();
            
            // Trouver les noms d'utilisateur
            let user_names: Vec<String> = chunk.iter().map(|t| {
                users.iter().find(|u| u.id == t.author_id)
                    .map(|u| u.name.clone())
                    .unwrap_or_else(|| "Unknown".to_string())
            }).collect();
            let user_name_refs: Vec<&str> = user_names.iter().map(|s| s.as_str()).collect();
            
            let user_screen_names: Vec<String> = chunk.iter().map(|t| {
                users.iter().find(|u| u.id == t.author_id)
                    .map(|u| u.username.clone())
                    .unwrap_or_else(|| format!("unknown_{}", t.author_id))
            }).collect();
            let user_screen_name_refs: Vec<&str> = user_screen_names.iter().map(|s| s.as_str()).collect();
            
            let texts: Vec<&str> = chunk.iter().map(|t| t.text.as_str()).collect();
            let sources: Vec<Option<&str>> = chunk.iter().map(|t| t.source.as_deref()).collect();
            let languages: Vec<&str> = chunk.iter().map(|t| t.lang.as_deref().unwrap_or("unknown")).collect();
            let possibly_sensitives: Vec<bool> = chunk.iter().map(|t| t.possibly_sensitive.unwrap_or(false)).collect();
            
            let retweet_counts: Vec<i64> = chunk.iter().map(|t| {
                t.public_metrics.as_ref().map(|m| m.retweet_count as i64).unwrap_or(0)
            }).collect();
            let reply_counts: Vec<i64> = chunk.iter().map(|t| {
                t.public_metrics.as_ref().map(|m| m.reply_count as i64).unwrap_or(0)
            }).collect();
            let quote_counts: Vec<i64> = chunk.iter().map(|t| {
                t.public_metrics.as_ref().map(|m| m.quote_count as i64).unwrap_or(0)
            }).collect();
            
            let query = format!(
                r#"INSERT INTO {}.tweet (id, created_at, published_time, user_id, user_name, user_screen_name, 
                   text, source, language, possibly_sensitive, retweet_count, reply_count, quote_count)
                SELECT * FROM UNNEST($1::text[], $2::text[], $3::bigint[], $4::text[], $5::text[], $6::text[],
                   $7::text[], $8::text[], $9::text[], $10::bool[], $11::bigint[], $12::bigint[], $13::bigint[])
                ON CONFLICT (id) DO NOTHING"#, self.schema_name
            );
            
            sqlx::query(&query)
                .bind(ids).bind(created_ats).bind(published_times).bind(user_ids)
                .bind(user_name_refs).bind(user_screen_name_refs).bind(texts).bind(sources)
                .bind(languages).bind(possibly_sensitives).bind(retweet_counts)
                .bind(reply_counts).bind(quote_counts)
                .execute(&self.pool).await
                .map_err(|e| WebError::WTFError(format!("Bulk insert tweets error: {}", e)))?;
        }
        Ok(())
    }

    async fn bulk_insert_replies(&self, replies: &[(&str, &str, &str, &str)]) -> Result<(), WebError> {
        if replies.is_empty() { return Ok(()); }
        
        const BATCH_SIZE: usize = 1000;
        for chunk in replies.chunks(BATCH_SIZE) {
            let tweet_ids: Vec<&str> = chunk.iter().map(|(tweet_id, _, _, _)| *tweet_id).collect();
            let reply_to_tweet_ids: Vec<&str> = chunk.iter().map(|(_, reply_to, _, _)| *reply_to).collect();
            let reply_to_user_ids: Vec<&str> = chunk.iter().map(|(_, _, user_id, _)| *user_id).collect();
            let screen_names: Vec<&str> = chunk.iter().map(|(_, _, _, screen_name)| *screen_name).collect();
            
            let query = format!(
                r#"INSERT INTO {}.reply (tweet_id, in_reply_to_tweet_id, in_reply_to_user_id, in_reply_to_screen_name)
                SELECT * FROM UNNEST($1::text[], $2::text[], $3::text[], $4::text[])
                ON CONFLICT (tweet_id) DO NOTHING"#, self.schema_name
            );
            
            sqlx::query(&query)
                .bind(tweet_ids).bind(reply_to_tweet_ids).bind(reply_to_user_ids).bind(screen_names)
                .execute(&self.pool).await
                .map_err(|e| WebError::WTFError(format!("Bulk insert replies error: {}", e)))?;
        }
        Ok(())
    }

    async fn bulk_insert_retweets(&self, retweets: &[(&str, &str)]) -> Result<(), WebError> {
        if retweets.is_empty() { return Ok(()); }
        
        const BATCH_SIZE: usize = 1000;
        for chunk in retweets.chunks(BATCH_SIZE) {
            let tweet_ids: Vec<&str> = chunk.iter().map(|(tweet_id, _)| *tweet_id).collect();
            let retweeted_ids: Vec<&str> = chunk.iter().map(|(_, retweeted)| *retweeted).collect();
            
            let query = format!(
                r#"INSERT INTO {}.retweet (tweet_id, retweeted_tweet_id)
                SELECT * FROM UNNEST($1::text[], $2::text[])
                ON CONFLICT (tweet_id) DO NOTHING"#, self.schema_name
            );
            
            sqlx::query(&query)
                .bind(tweet_ids).bind(retweeted_ids)
                .execute(&self.pool).await
                .map_err(|e| WebError::WTFError(format!("Bulk insert retweets error: {}", e)))?;
        }
        Ok(())
    }

    async fn bulk_insert_quotes(&self, quotes: &[(&str, &str)]) -> Result<(), WebError> {
        if quotes.is_empty() { return Ok(()); }
        
        const BATCH_SIZE: usize = 1000;
        for chunk in quotes.chunks(BATCH_SIZE) {
            let tweet_ids: Vec<&str> = chunk.iter().map(|(tweet_id, _)| *tweet_id).collect();
            let quoted_ids: Vec<&str> = chunk.iter().map(|(_, quoted)| *quoted).collect();
            
            let query = format!(
                r#"INSERT INTO {}.quote (tweet_id, quoted_tweet_id)
                SELECT * FROM UNNEST($1::text[], $2::text[])
                ON CONFLICT (tweet_id) DO NOTHING"#, self.schema_name
            );
            
            sqlx::query(&query)
                .bind(tweet_ids).bind(quoted_ids)
                .execute(&self.pool).await
                .map_err(|e| WebError::WTFError(format!("Bulk insert quotes error: {}", e)))?;
        }
        Ok(())
    }

    async fn bulk_insert_tweet_places(&self, tweet_places: &[(&str, &str)]) -> Result<(), WebError> {
        if tweet_places.is_empty() { return Ok(()); }
        
        const BATCH_SIZE: usize = 1000;
        for chunk in tweet_places.chunks(BATCH_SIZE) {
            let tweet_ids: Vec<&str> = chunk.iter().map(|(tweet_id, _)| *tweet_id).collect();
            let place_ids: Vec<&str> = chunk.iter().map(|(_, place_id)| *place_id).collect();
            
            let query = format!(
                r#"INSERT INTO {}.tweet_place (tweet_id, place_id)
                SELECT * FROM UNNEST($1::text[], $2::text[])
                ON CONFLICT (tweet_id, place_id) DO NOTHING"#, self.schema_name
            );
            
            sqlx::query(&query)
                .bind(tweet_ids).bind(place_ids)
                .execute(&self.pool).await
                .map_err(|e| WebError::WTFError(format!("Bulk insert tweet places error: {}", e)))?;
        }
        Ok(())
    }

    async fn bulk_insert_corpus(&self, corpus: &[(&str, &str)]) -> Result<(), WebError> {
        if corpus.is_empty() { return Ok(()); }
        
        const BATCH_SIZE: usize = 1000;
        for chunk in corpus.chunks(BATCH_SIZE) {
            let tweet_ids: Vec<&str> = chunk.iter().map(|(tweet_id, _)| *tweet_id).collect();
            let texts: Vec<&str> = chunk.iter().map(|(_, text)| *text).collect();
            
            let query = format!(
                r#"INSERT INTO {}.corpus (tweet_id, corpus)
                SELECT * FROM UNNEST($1::text[], $2::text[])
                ON CONFLICT (tweet_id) DO NOTHING"#, self.schema_name
            );
            
            sqlx::query(&query)
                .bind(tweet_ids).bind(texts)
                .execute(&self.pool).await
                .map_err(|e| WebError::WTFError(format!("Bulk insert corpus error: {}", e)))?;
        }
        Ok(())
    }

    async fn bulk_insert_withheld(&self, withheld: &[(&str, &str)]) -> Result<(), WebError> {
        if withheld.is_empty() { return Ok(()); }
        
        const BATCH_SIZE: usize = 1000;
        for chunk in withheld.chunks(BATCH_SIZE) {
            let user_ids: Vec<&str> = chunk.iter().map(|(user_id, _)| *user_id).collect();
            let countries: Vec<&str> = chunk.iter().map(|(_, country)| *country).collect();
            
            let query = format!(
                r#"INSERT INTO {}.withheld_in_country (user_id, country)
                SELECT * FROM UNNEST($1::text[], $2::text[])
                ON CONFLICT (user_id, country) DO NOTHING"#, self.schema_name
            );
            
            sqlx::query(&query)
                .bind(user_ids).bind(countries)
                .execute(&self.pool).await
                .map_err(|e| WebError::WTFError(format!("Bulk insert withheld error: {}", e)))?;
        }
        Ok(())
    }

    async fn bulk_insert_emojis(&self, emojis: &[(&str, &str, i32)]) -> Result<(), WebError> {
        if emojis.is_empty() { return Ok(()); }
        
        for chunk in emojis.chunks(1000) {
            let tweet_ids: Vec<&str> = chunk.iter().map(|(tweet_id, _, _)| *tweet_id).collect();
            let emoji_chars: Vec<&str> = chunk.iter().map(|(_, emoji, _)| *emoji).collect();
            let orders: Vec<i32> = chunk.iter().map(|(_, _, order)| *order).collect();
            
            let query = format!(
                r#"INSERT INTO {}.tweet_emoji (tweet_id, emoji, "order")
                SELECT * FROM UNNEST($1::text[], $2::text[], $3::int[])
                ON CONFLICT (tweet_id, emoji) DO NOTHING"#, self.schema_name
            );
            
            sqlx::query(&query)
                .bind(tweet_ids).bind(emoji_chars).bind(orders)
                .execute(&self.pool).await
                .map_err(|e| WebError::WTFError(format!("Bulk insert emojis error: {}", e)))?;
        }
        Ok(())
    }

    async fn bulk_insert_hashtags(&self, hashtags: &[(&str, &str, i32, usize, usize)], batch_size: usize) -> Result<(), WebError> {
        if hashtags.is_empty() { return Ok(()); }
        
        for chunk in hashtags.chunks(batch_size) {
            let tweet_ids: Vec<&str> = chunk.iter().map(|(tweet_id, _, _, _, _)| *tweet_id).collect();
            let hashtag_tags: Vec<&str> = chunk.iter().map(|(_, hashtag, _, _, _)| *hashtag).collect();
            let orders: Vec<i32> = chunk.iter().map(|(_, _, order, _, _)| *order).collect();
            let starts: Vec<i32> = chunk.iter().map(|(_, _, _, start, _)| *start as i32).collect();
            let ends: Vec<i32> = chunk.iter().map(|(_, _, _, _, end)| *end as i32).collect();
            
            let query = format!(
                r#"INSERT INTO {}.tweet_hashtag (tweet_id, hashtag, "order", start_indice, end_indice)
                SELECT * FROM UNNEST($1::text[], $2::text[], $3::int[], $4::int[], $5::int[])
                ON CONFLICT (tweet_id, hashtag) DO NOTHING"#, self.schema_name
            );
            
            sqlx::query(&query)
                .bind(tweet_ids).bind(hashtag_tags).bind(orders).bind(starts).bind(ends)
                .execute(&self.pool).await
                .map_err(|e| WebError::WTFError(format!("Bulk insert hashtags error: {}", e)))?;
        }
        Ok(())
    }

    async fn bulk_insert_urls(&self, urls: &[(&str, &str, i32, usize, usize)], batch_size: usize) -> Result<(), WebError> {
        if urls.is_empty() { return Ok(()); }
        
        for chunk in urls.chunks(batch_size) {
            let tweet_ids: Vec<&str> = chunk.iter().map(|(tweet_id, _, _, _, _)| *tweet_id).collect();
            let url_strings: Vec<&str> = chunk.iter().map(|(_, url, _, _, _)| *url).collect();
            let orders: Vec<i32> = chunk.iter().map(|(_, _, order, _, _)| *order).collect();
            let starts: Vec<i32> = chunk.iter().map(|(_, _, _, start, _)| *start as i32).collect();
            let ends: Vec<i32> = chunk.iter().map(|(_, _, _, _, end)| *end as i32).collect();
            
            let query = format!(
                r#"INSERT INTO {}.tweet_url (tweet_id, url, "order", start_indice, end_indice)
                SELECT * FROM UNNEST($1::text[], $2::text[], $3::int[], $4::int[], $5::int[])
                ON CONFLICT (tweet_id, url) DO NOTHING"#, self.schema_name
            );
            
            sqlx::query(&query)
                .bind(tweet_ids).bind(url_strings).bind(orders).bind(starts).bind(ends)
                .execute(&self.pool).await
                .map_err(|e| WebError::WTFError(format!("Bulk insert URLs error: {}", e)))?;
        }
        Ok(())
    }

    async fn bulk_insert_cashtags(&self, cashtags: &[(&str, String, i32, usize, usize)], batch_size: usize) -> Result<(), WebError> {
        if cashtags.is_empty() { return Ok(()); }
        
        for chunk in cashtags.chunks(batch_size) {
            let tweet_ids: Vec<&str> = chunk.iter().map(|(tweet_id, _, _, _, _)| *tweet_id).collect();
            let cashtag_strings: Vec<&str> = chunk.iter().map(|(_, cashtag, _, _, _)| cashtag.as_str()).collect();
            let orders: Vec<i32> = chunk.iter().map(|(_, _, order, _, _)| *order).collect();
            let starts: Vec<i32> = chunk.iter().map(|(_, _, _, start, _)| *start as i32).collect();
            let ends: Vec<i32> = chunk.iter().map(|(_, _, _, _, end)| *end as i32).collect();
            
            let query = format!(
                r#"INSERT INTO {}.tweet_cashtag (tweet_id, cashtag, "order", start_indice, end_indice)
                SELECT * FROM UNNEST($1::text[], $2::text[], $3::int[], $4::int[], $5::int[])
                ON CONFLICT (tweet_id, cashtag) DO NOTHING"#, self.schema_name
            );
            
            sqlx::query(&query)
                .bind(tweet_ids).bind(cashtag_strings).bind(orders).bind(starts).bind(ends)
                .execute(&self.pool).await
                .map_err(|e| WebError::WTFError(format!("Bulk insert cashtags error: {}", e)))?;
        }
        Ok(())
    }

    async fn bulk_insert_mentions(&self, mentions: &[(&str, &str, i32, usize, usize)], batch_size: usize) -> Result<(), WebError> {
        if mentions.is_empty() { return Ok(()); }
        
        for chunk in mentions.chunks(batch_size) {
            let tweet_ids: Vec<&str> = chunk.iter().map(|(tweet_id, _, _, _, _)| *tweet_id).collect();
            let user_ids: Vec<&str> = chunk.iter().map(|(_, user_id, _, _, _)| *user_id).collect();
            let orders: Vec<i32> = chunk.iter().map(|(_, _, order, _, _)| *order).collect();
            let starts: Vec<i32> = chunk.iter().map(|(_, _, _, start, _)| *start as i32).collect();
            let ends: Vec<i32> = chunk.iter().map(|(_, _, _, _, end)| *end as i32).collect();
            
            let query = format!(
                r#"INSERT INTO {}.tweet_user_mention (tweet_id, user_id, "order", start_indice, end_indice)
                SELECT * FROM UNNEST($1::text[], $2::text[], $3::int[], $4::int[], $5::int[])
                ON CONFLICT (tweet_id, user_id) DO NOTHING"#, self.schema_name
            );
            
            sqlx::query(&query)
                .bind(tweet_ids).bind(user_ids).bind(orders).bind(starts).bind(ends)
                .execute(&self.pool).await
                .map_err(|e| WebError::WTFError(format!("Bulk insert mentions error: {}", e)))?;
        }
        Ok(())
    }

    async fn bulk_insert_media(&self, tweets: &[TwitterTweet], media_list: &[TwitterMedia]) -> Result<(), WebError> {
        if media_list.is_empty() { return Ok(()); }
        
        let mut tweet_media_pairs = Vec::new();
        
        // Associer les médias aux tweets via les entités URL avec media_key
        for tweet in tweets {
            if let Some(entities) = &tweet.entities {
                if let Some(urls) = &entities.urls {
                    for (order, url) in urls.iter().enumerate() {
                        if let Some(media_key) = &url.media_key {
                            if let Some(media) = media_list.iter().find(|m| &m.media_key == media_key) {
                                let media_url = media.url.as_deref().unwrap_or("unknown");
                                let media_type = media.media_type.as_str();
                                tweet_media_pairs.push((tweet.id.as_str(), media_url, media_type, order as i32));
                            }
                        }
                    }
                }
            }
        }
        
        for chunk in tweet_media_pairs.chunks(1000) {
            let tweet_ids: Vec<&str> = chunk.iter().map(|(tweet_id, _, _, _)| *tweet_id).collect();
            let media_urls: Vec<&str> = chunk.iter().map(|(_, url, _, _)| *url).collect();
            let media_types: Vec<&str> = chunk.iter().map(|(_, _, type_field, _)| *type_field).collect();
            let orders: Vec<i32> = chunk.iter().map(|(_, _, _, order)| *order).collect();
            
            let query = format!(
                r#"INSERT INTO {}.tweet_media (tweet_id, media_url, type, "order")
                SELECT * FROM UNNEST($1::text[], $2::text[], $3::text[], $4::int[])
                ON CONFLICT (tweet_id, media_url) DO NOTHING"#, self.schema_name
            );
            
            sqlx::query(&query)
                .bind(tweet_ids).bind(media_urls).bind(media_types).bind(orders)
                .execute(&self.pool).await
                .map_err(|e| WebError::WTFError(format!("Bulk insert media error: {}", e)))?;
        }
        Ok(())
    }
} 