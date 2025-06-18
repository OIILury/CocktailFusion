# Guide de Test des Optimisations de Collecte

## Résumé des Optimisations Implementées

### 1. **Parallélisation Massive**
- ✅ Collecte simultanée de tous les mots-clés sur tous les réseaux
- ✅ Tasks Tokio parallèles au lieu d'un traitement séquentiel
- ✅ Performance théorique : 10x plus rapide pour 5 mots-clés sur 2 réseaux

### 2. **Optimisation des Requêtes API**
- ✅ Twitter : Batches de 100 tweets par requête (maximum API)
- ✅ Bluesky : Batches de 25 posts par requête (maximum API)
- ✅ Délais réduits : 20ms pour gros volumes, 50ms pour volumes normaux
- ✅ Calcul dynamique du nombre max de requêtes basé sur la limite

### 3. **Traitement en Batch des Insertions DB**
- ✅ Nouvelles méthodes `save_tweets_batch_to_db()` et `save_posts_batch_to_db()`
- ✅ Transactions par batches de 50 tweets/posts
- ✅ Réduction drastique des connexions DB (de N à N/50)

### 4. **Logging et Monitoring**
- ✅ Métriques de performance par tâche
- ✅ Calcul de posts/seconde
- ✅ Logs détaillés pour debugging

## Tests de Performance Recommandés

### Test 1 : Volume Modéré (Baseline)
```json
{
    "name": "Test_Baseline",
    "keywords": ["test", "performance"],
    "networks": ["twitter", "bluesky"],
    "limit": 100
}
```
**Attendu :** ~4-8 secondes pour 400 posts

### Test 2 : Volume Important
```json
{
    "name": "Test_1K",
    "keywords": ["sport", "tech", "news"],
    "networks": ["twitter", "bluesky"], 
    "limit": 1000
}
```
**Attendu :** ~30-60 secondes pour 6000 posts

### Test 3 : Volume Massif
```json
{
    "name": "Test_10K",
    "keywords": ["breaking", "today", "update", "news", "trending"],
    "networks": ["twitter", "bluesky"],
    "limit": 10000
}
```
**Attendu :** ~5-10 minutes pour 100,000 posts

## Métriques à Surveiller

### Performance API
- Requêtes par seconde
- Taux d'erreur API
- Temps de réponse moyen

### Performance DB
- Temps d'insertion par batch
- Nombre de transactions
- Taux de commit/rollback

### Performance Globale
- Posts collectés par seconde
- Temps total de collecte
- Utilisation mémoire

## Configuration Recommandée pour Gros Volumes

### Variables d'Environnement
```bash
# Pool de connexions DB plus important
PG_MAX_CONNECTIONS=50

# Timeouts étendus
HTTP_TIMEOUT=300

# Logging détaillé
RUST_LOG=cocktail_server=info,tower_http=debug
```

### Paramètres Optimaux par Volume
- **< 1,000 posts :** Limite standard (10-100)
- **1,000-10,000 posts :** Limite 1000, délai 50ms
- **10,000-100,000 posts :** Limite 10000, délai 20ms  
- **100,000+ posts :** Limite 50000+, délai 10ms

## Exemple de Logs de Performance Optimisés

```
[INFO] Starting 10 parallel collection tasks
[INFO] Starting collection for network: twitter, keyword: news, limit: 10000
[INFO] Configured for max 150 requests to collect 10000 tweets
[INFO] Found 100 Twitter tweets for keyword: news
[INFO] Batch of 50 tweets processed, total saved: 50
[INFO] Successfully saved 97/100 Twitter tweets for keyword: news
[INFO] Collection completed for twitter/news: 97 posts in 2.3s (42.17 posts/sec)
```

## Améliorations Futures Possibles

1. **Connection Pooling** : Réutiliser les connexions HTTP
2. **Streaming** : Traitement au fur et à mesure des réponses API
3. **Cache** : Éviter les doublons de tweets
4. **Rate Limiting Intelligent** : Adaptation dynamique aux limites API
5. **Compression** : Réduire la taille des données en mémoire 