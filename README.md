# Université de Bourgogne - Cocktail front

-   **Client** : Université de Bourgogne
-   **Objet** : ...

## Organisation des sources

-   <del>**cocktail-js**</del> : OBSOLETE

-   **e2e-cypress** : tests de...

-   **server**
    -   **authentication**
        -   `justfile`
    -   **cli** : production du binaire cocktail
        -   `justfile`
    -   **cocktail-db-twitter** : chercher les topk et hashtags ("Les plus utilisés" + "Rechercher")
    -   topk.db : fichier SQLite
    -   **cocktail-db-web** : ...
    -   **cocktail-graph-utils** : lancer les fichiers python
    -   **cocktail-server** : serveur web proprement dit (routes, etc.)
    -   **cocktail-twitter-data** : ...
    -   **fts** "full text search": moteur de recherche plain-text similaire à elasticsearch en rust
    -   **kratos** : clone du projet web
    -   **scripts-ub** : python / R
    -   **tantivy-data** : répertoire de données
    -   `Jenkinsfile.release` : ...
    -   `justfile`
-   <del>`Jenkinsfile`</del> : OBSOLETE ?
-   <del>`Jenkinsfile-release`</del> : OBSOLETE ?

## Serveurs de démo

-   https://ub-cocktail.atolcd.show
-   https://ub-cocktail-graph.atolcd.show
-   https://ub-cocktail-auth.atolcd.show

## Prérequis

-   [nvm](https://github.com/nvm-sh/nvm#installing-and-updating)

-   [rust](https://rustup.rs/)

-   [ory/kratos](https://www.ory.sh/docs/kratos/install) comme serveur d'authentification sans interface utilisateur
    -   nécessaire si on n'utilise pas les tâches just kratos <code>docker-\*</code>

<ul><li style="list-style-type:none;"><ul><li><details>
<summary>exemple Linux</summary>

On se place dans /tmp pour ne pas s'embêter avec les droits puis on déplace kratos dans le path :

```javascript
(cd /tmp && bash <(curl https://raw.githubusercontent.com/ory/meta/master/install.sh) -d -b . kratos v0.10.1 && sudo mv ./kratos /usr/local/bin/)
```

Puis redémarrer le shell

</details></li></ul></li></ul>

-   [just](https://github.com/casey/just#packages) pour exécuter des commandes (semblable à makefile)

-   [gunzip](https://www.gnu.org/software/gzip/) pour décompresser des fichiers

## Autres documentations

-   [Convertir la base Postgre en JSON](server/tweets-from-sql-to-json/README.md)

-   [Kratos](server/kratos/README.md)

## Démarrage

Les commandes sont exécutées dans le répertoire racine du projet.

### Dépendances

```sh
# node 12
nvm install v12
nvm use v12
```

> **Voir par rapport à la volumétrie entre autres :**
>
> -   composition docker en local et un jeu de données light ?
> -   accéder à la base de données d'un serveur de démo ?
> -   ...

Ouvrir un tunnel pour accéder à la base de données :

```sh
ssh -L 5432:127.0.0.1:5432 ub-cocktail-dev.dvt.cloud.priv.atolcd.com
```

### Démarrage des applications

#### 1. Exécution du serveur d'authentification

```sh
cd server/authentication

# Migration  (ou migrate)
just docker-migrate

# Exécution (ou kratos)
just docker-kratos
```

#### 2. Exécution de l'application Cocktail (profil debug)

Déposer `tweet_with_metrics-100000.json.gz` dans le répertoire `server`.

```sh
cd server

# Construction cocktail, doc_count et topk
just build-all-debug

# Aide cocktail
./target/debug/cocktail help
```

Créer et alimenter l'index tantivy

```sh
# Création du répertoire d'indexation (une fois)
./target/debug/cocktail index create --directory-path tantivy-data

# Ingestion des indexes des tweets (contenu de tantivy-data)
gunzip -c tweet_with_metrics-100000.json.gz | ./target/debug/cocktail index ingest --directory-path tantivy-data
```

Calculer les topk :

-   les hashtags les plus populaires
-   les tweets les plus commentés / les plus retweetés
-   les tendances du moment (plus forte progression...)

```sh
# json vers sqlite (ou topk-db)
just docker-topk-db
# si profil debug
just profile=debug docker-topk-db
```

Créer les fichiers topk :

```
./target/debug/topk --directory-path ./tantivy-data/ --query "*" | sqlite-utils insert --not-null key --not-null doc_count topk.db hashtag -

# Topk de cooccurence
./target/debug/topk_cooccurence --pg-database-url postgres://cocktailuser:cocktailuser@localhost:5432/cocktail_pg --schema vegan_pac | sqlite-utils insert --not-null hashtag1 --not-null hashtag2 --not-null count topk.db hashtag_cooccurence -

```

Installation dépendance cocktail-serve et build css :

```sh
cd cocktail-server
npm install
just build-css
cd ..
```

Creation des dossiers utilisés par l'app

```sh
mkdir log

mkdir project-data
```

Lancer cocktail :

```sh
just profile=debug serve
```

Lancer cocktail en debug en mode watch :
Necessite d'avoir installé le package [cargo-watch](https://crates.io/crates/cargo-watch). Cette commande recompile les scss à chaque changement de scss et recompile le serveur à chaque changement de code.

```sh
just watch-dev
```

Ouvrir la page : http://localhost:3000/

Créer le compte pour passer à la suite :

-   http://localhost:3000/auth/registration
-   Exemple : `john@example.com` / `L'Observatoire en temps réel des tendances`

## Surcharge de la configuration

### Configuration

> Voir sir ce qui est indiqué est juste.

Surcharger les variables d'environnement des fichiers `.env`.

**Serveur cocktail :**

-   **`server/.env`**
-   `server/cli/.env`. Cf. `server/cli/src/main.rs`
-   `server/cocktail-db-web/.env`
-   `server/cocktail-db-twitter/.env`
-   `server/cocktail-server/.env`

**Serveur d'authenfification :**

-   `server/authentication/.env`
-   `server/authentication/kratos.yml`

### Journalisation

> Voir sir ce qui est indiqué est juste.

**Serveur cocktail :**

-   Définir la variable [`RUST_LOG`](https://rust-lang-nursery.github.io/rust-cookbook/development_tools/debugging/config_log.html) ?

**Serveur d'authentification :**

-   `server/authentication/kratos.yml`

```yml
log:
    level: debug
    format: text
    leak_sensitive_values: true
```

## Opérations sur la base de données

Bases `SQLite` ([documentation du cli](https://sqlite-utils.datasette.io/en/stable/cli.html)) :

```sh
# Création d'un alias
sqlite-utils() {
  docker run --rm --user "$(id -u)":"$(id -g)" -v "$PWD":/usr/src/myapp -w /usr/src/myapp ghcr.io/williamjacksn/sqlite-utils:latest "$@"
}

sqlite-utils --help

# Lister les tables de la base d'utilisateurs
sqlite-utils tables authentication/data.db

# Obtenir le contenu d'une table
sqlite-utils rows authentication/data.db identity_credential_identifiers
```

## Documentation

### Conventions de développement

> ...

### Journal des changements

> ...

### Journal des décisions d'architecture (ADRs)

> ...

### Documentation d'exploitation

> ...

### Commandes utiles

Creation d'une migration:

```sh
cd server/cocktail-db-web
sqlx migrate add -r nom_de_ma_migration
```

Application des migrations:

```sh
cd server/cocktail-db-web
sqlx migrate run
```

C'est pas beau ? Pour build le scss:

```sh
cd  server/cocktail-server
just build-css
```

#### Déploiement

Le déploiement sur le serveur de démo est un `scp` du binaire sur le serveur de démo.

_Il faut lancer_

```sh
cd server/
just prepare
```

_si cela n'a pas été fait et `commit` le résultat_

Le binaire est donc construit en local (_bien penser à n'avoir rien en cours, et à bien `commit` les changements qui ne le seraient pas_)

Avant toute chose, il faut arrêter sur le serveur de démo le serveur qui tourne avec les étapes qui suivent:

```sh
ssh  ub-cocktail-dev.dvt.cloud.priv.atolcd.com
```

On est maintenant sur le serveur de démo. On reprend la main sur le serveur:

```sh
screen -r cocktail
```

Si la commande précédente ne fonctionne pas c'est qu'aucun `screen` ne tourne et donc cocktail est probablement KO.
Les commandes pour cette situation sont (en ssh sur le serveur):

```sh
cd /usr/local/lib
screen -S kratos # On crée le screen plutot que de s'attacher a un screen existant
cd  authentication/
just serve # L'authentification est accessible
```

`ctrl+a ctrl+d` pour mettre le `screen` en arrière-plan.

```sh
screen -S cocktail
# just serve pour le lancement si besoin
```

On coupe le serveur sur le screen cocktail avec `ctrl+c` (si il tournait)
**Le serveur est arrêté.**

Depuis le **local**

```sh
cd server/
SQLX_OFFLINE=true just deploy cocktail
```

On peut maintenant depuis le terminal en **SSH** relancer le serveur. Dans le `screen` il faut (sinon `screen -r cocktail`) faire:

```sh
just serve
```

`ctrl+a ctrl+d` pour mettre le `screen` en arrière-plan.

Le site devrait maintenant être accessible.

## Intégration de la collecte vaccination

Connexion au serveur en SSH

```sh
ssh  ub-cocktail-dev.dvt.cloud.priv.atolcd.com
```

On coupe le serveur cocktail le temps de faire la manipulation si besoin:

```sh
cd /usr/local/lib
screen -r cocktail
# CTRL + C
```

On coupe le serveur avec `ctrl+c`
Se connecter sur le `screen` cocktail-data pour supprimer et régénerer la base FTS

```sh
cd /usr/local/lib
screen -r cocktail-data # screen -S cocktail-data si le screen n'existe pas
echo "XXXX-XX-XX" | ./tweets-from-sql-to-json | gzip -c > collecte/tweets_collecte_XXXX_XX_XX.json.gz # Generer le JSON depuis la BDD pour la date donnée en paramètre
# Si besoin pour régenerer la base FTS
# rm -rf full-text-data/ # Supprime la BDD FTS actuelle
# ./cocktail index create --directory-path full-text-data # Crée le nouvel index
gunzip -c collecte/tweets_collecte_XXXX_XX_XX.json.gz | ./cocktail index ingest  --directory-path full-text-data # Integre le nouveau JSON
rm topk.db # Supprime la base topk contentnant les hashtags les plus utilisés
./topk --directory-path ./full-text-data/ --query "*" | sqlite-utils insert --not-null key --not-null doc_count topk.db hashtag - # Génére la nouvelle base topk
./topk_cooccurence --pg-database-url postgres://cocktailuser:cocktailuser@localhost:5432/cocktail_pg --schema vegan_pac | sqlite-utils insert --not-null hashtag1 --not-null hashtag2 --not-null count topk.db hashtag_cooccurence -
```

CTRL + A - D pour quitter le `screen` et relance `cocktail`

```sh
screen -r cocktail
just serve
```

Commandes utiles:

Genère les fichier entre deux dates:
(Borne inférieure incluse et superieure excluse)

```sh
d=2020-04-10
while [ "$d" != 2020-12-01 ]; do
  echo $d | ./tweets-from-sql-to-json | gzip -c > collecte/tweets_collecte_$d.json.gz
  d=$(date -I -d "$d + 1 day")

done
```

Intègre entre deux dates:
(Borne inférieure incluse et superieure excluse)

```sh
d=2020-04-10
while [ "$d" != 2020-12-01 ]; do
  gunzip -c collecte/tweets_collecte_$d.json.gz | ./cocktail index ingest  --directory-path full-text-data
  d=$(date -I -d "$d + 1 day")
done
```

## Suppression des études anonymes

Les études créées par des utilisateurs non enregistrés sont supprimées si elles n'ont pas été modifiées depuis plus de 24h
Via cette commande

```sh
# Si besoin en dev just profile=debug study clear
just study clear
```

A mettre dans un cron si possible
