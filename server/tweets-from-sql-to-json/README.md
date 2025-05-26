# build

dans ce répertoire :

```
cargo build --release
```

dans le répertoire parent :

```
cargo build --release --bin tweets-from-sql-to-json
```

# utilisation

```
./target/release/tweets-from-sql-to-json | gzip -c > tweet_with_metrics.json.gz
```

(ou `../target/[…]` si on est dans le répertoire parent)

Sur le serveur de démo:

Pour déployer le `bin` si besoin:

```
just deploy tweets-from-sql-to-json
```

Puis pour l'utiliser:

```
./tweets-from-sql-to-json | gzip -c > tweets_collecte.json.gz
```
