# Guide d'Installation de Cocktail Front sur Windows

Ce guide vous permettra d'installer et de configurer l'application Cocktail Front sur votre système Windows.

## Prérequis

1. **Git**
   - Téléchargez et installez Git depuis : https://git-scm.com/download/windows
   - Pendant l'installation, choisissez "Git from the command line and also from 3rd-party software"

2. **Node.js**
   - Téléchargez et installez Node.js v12 depuis : https://nodejs.org/dist/latest-v12.x/
   - Vérifiez l'installation en ouvrant PowerShell et en tapant :
     ```powershell
     node --version
     npm --version
     ```

3. **Rust**
   - Ouvrez PowerShell et exécutez :
     ```powershell
     winget install Rustlang.Rust.MSVC
     ```
   - Ou visitez https://rustup.rs/ et suivez les instructions
   - Redémarrez PowerShell après l'installation
   - Vérifiez l'installation :
     ```powershell
     rustc --version
     cargo --version
     ```

4. **PostgreSQL**
   - Téléchargez et installez PostgreSQL depuis : https://www.postgresql.org/download/windows/
   - Pendant l'installation :
     - Notez le mot de passe que vous définissez pour l'utilisateur 'postgres'
     - Laissez le port par défaut (5432)
   - Ajoutez PostgreSQL aux variables d'environnement système :
     - Recherchez "variables d'environnement" dans Windows
     - Dans "Variables système", modifiez "Path"
     - Ajoutez : `C:\Program Files\PostgreSQL\14\bin` (adaptez selon votre version)

5. **Just**
   - Installez Just via cargo :
     ```powershell
     cargo install just
     ```

## Installation

1. **Cloner le projet**
   ```powershell
   git clone https://github.com/votre-repo/cocktail-front.git
   cd cocktail-front
   ```

2. **Configuration de la base de données**
   ```powershell
   # Connectez-vous à PostgreSQL
   psql -U postgres
   
   # Créez la base de données et l'utilisateur
   CREATE DATABASE cocktail_pg;
   CREATE USER cocktailuser WITH PASSWORD 'cocktailuser';
   GRANT ALL PRIVILEGES ON DATABASE cocktail_pg TO cocktailuser;
   
   # Quittez psql
   \q
   ```

3. **Configuration du serveur d'authentification**
   ```powershell
   cd server/authentication
   
   # Démarrez les conteneurs Docker
   just docker-migrate
   just docker-kratos
   ```

4. **Configuration de l'application**
   ```powershell
   # Retournez à la racine du projet
   cd ../..
   
   # Créez le fichier .env dans le dossier server
   echo "DATABASE_URL=postgres://cocktailuser:cocktailuser@localhost:5432/cocktail_pg" > server/.env
   ```

5. **Compilation et démarrage**
   ```powershell
   cd server
   
   # Construction des composants
   just build-all-debug
   
   # Création de l'index
   mkdir tantivy-data
   ./target/debug/cocktail index create --directory-path tantivy-data
   
   # Si vous avez un fichier de données tweets
   gunzip -c tweet_with_metrics-100000.json.gz | ./target/debug/cocktail index ingest --directory-path tantivy-data
   
   # Installation des dépendances du serveur web
   cd cocktail-server
   npm install
   just build-css
   cd ..
   
   # Création des dossiers nécessaires
   mkdir log
   mkdir project-data
   
   # Démarrage du serveur
   just profile=debug serve
   ```

## Accès à l'application

1. Ouvrez votre navigateur et accédez à : http://localhost:3000/

2. Créez un compte :
   - Accédez à http://localhost:3000/auth/registration
   - Remplissez le formulaire d'inscription
   - Exemple : email@example.com / MotDePasse123!

## Dépannage

1. **Erreur de port PostgreSQL**
   - Vérifiez que PostgreSQL est en cours d'exécution :
     ```powershell
     net start postgresql-x64-14
     ```
   - Vérifiez le port dans pg_hba.conf

2. **Erreur de compilation Rust**
   - Nettoyez et reconstruisez :
     ```powershell
     cargo clean
     cargo build
     ```

3. **Erreur de dépendances Node.js**
   - Supprimez node_modules et réinstallez :
     ```powershell
     rm -r node_modules
     npm install
     ```

## Notes importantes

- Assurez-vous que tous les services (PostgreSQL, Kratos) sont en cours d'exécution avant de démarrer l'application
- Pour arrêter l'application, utilisez Ctrl+C dans le terminal
- Les logs sont disponibles dans le dossier `log`
- Pour redémarrer l'application, utilisez `just profile=debug serve`

## Commandes utiles

- Redémarrer PostgreSQL : `net restart postgresql-x64-14`
- Vérifier les services en cours : `net start`
- Nettoyer l'index : `rm -r tantivy-data/*`
- Reconstruire le CSS : `cd server/cocktail-server && just build-css` 