console.log('Collect.js chargé!');

// Fonction d'initialisation pour une meilleure gestion des erreurs
function initCollect() {
    console.log('Initialisation de la collecte...');
    
    // Récupération des éléments du DOM
    const addKeywordBtn = document.getElementById('addKeyword');
    const keywordInput = document.getElementById('keywords');
    const keywordsList = document.getElementById('keywordsList');
    const startCollectBtn = document.getElementById('startCollect');
    const collectNameInput = document.getElementById('collectName');
    
    // Vérification que les éléments sont bien trouvés
    console.log('Éléments trouvés:', {
        addKeywordBtn: !!addKeywordBtn,
        keywordInput: !!keywordInput,
        keywordsList: !!keywordsList,
        startCollectBtn: !!startCollectBtn,
        collectNameInput: !!collectNameInput
    });
    
    // Si un élément essentiel est manquant, arrêter l'exécution
    if (!startCollectBtn) {
        console.error("Le bouton startCollect n'a pas été trouvé!");
        return;
    }
    
    if (!keywordInput || !keywordsList || !addKeywordBtn || !collectNameInput) {
        console.error("Un ou plusieurs éléments nécessaires n'ont pas été trouvés!");
        return;
    }
    
    // Get project ID from the URL
    const pathSegments = window.location.pathname.split('/');
    const projectId = pathSegments[pathSegments.indexOf('projets') + 1];
    console.log('Project ID:', projectId);
    
    // Array to store keywords
    let keywords = [];
    
    // Add delete collection button handler
    const deleteCollectBtn = document.getElementById('deleteCollect');
    if (deleteCollectBtn) {
        deleteCollectBtn.addEventListener('click', async function(e) {
            console.log('Bouton de suppression cliqué!');
            
            if (!confirm('Êtes-vous sûr de vouloir supprimer toutes les données collectées ? Cette action est irréversible.')) {
                return;
            }
            
            try {
                // Disable button and show loading state
                deleteCollectBtn.disabled = true;
                deleteCollectBtn.textContent = 'Suppression en cours...';
                
                const response = await fetch(`/projets/${projectId}/collect/delete`, {
                    method: 'POST',
                    headers: {
                        'Content-Type': 'application/json'
                    }
                });
                
                const data = await response.json();
                
                if (data.success) {
                    alert('Les données ont été supprimées avec succès');
                    window.location.reload();
                } else {
                    alert(`Erreur: ${data.message}`);
                }
            } catch (error) {
                console.error('Erreur lors de la suppression:', error);
                alert(`Une erreur est survenue lors de la suppression: ${error.message}`);
            } finally {
                // Re-enable button
                deleteCollectBtn.disabled = false;
                deleteCollectBtn.textContent = 'Supprimer les données collectées';
            }
        });
    }
    
    // Add update data button handler
    const updateDataBtn = document.getElementById('updateData');
    if (updateDataBtn) {
        updateDataBtn.addEventListener('click', async function(e) {
            console.log('Bouton de mise à jour cliqué!');
            
            if (!confirm('Êtes-vous sûr de vouloir mettre à jour toutes les données ? Cette opération peut prendre plusieurs minutes.')) {
                return;
            }
            
            try {
                // Disable button and show loading state
                updateDataBtn.disabled = true;
                updateDataBtn.textContent = 'Mise à jour en cours...';
                
                const response = await fetch(`/projets/${projectId}/collect/update`, {
                    method: 'POST',
                    headers: {
                        'Content-Type': 'application/json'
                    }
                });
                
                const data = await response.json();
                
                if (data.success) {
                    alert('Les données ont été mises à jour avec succès');
                    window.location.reload();
                } else {
                    alert(`Erreur: ${data.message}`);
                }
            } catch (error) {
                console.error('Erreur lors de la mise à jour:', error);
                alert(`Une erreur est survenue lors de la mise à jour: ${error.message}`);
            } finally {
                // Re-enable button
                updateDataBtn.disabled = false;
                updateDataBtn.textContent = 'Mettre à jour les données';
            }
        });
    }
    
    // Function to add keyword to the list
    function addKeyword() {
        const keyword = keywordInput.value.trim();
        console.log('Tentative d\'ajout du mot-clé:', keyword);
        
        if (keyword && !keywords.includes(keyword)) {
            keywords.push(keyword);
            
            // Create keyword element
            const keywordElement = document.createElement('div');
            keywordElement.className = 'keyword-item';
            keywordElement.innerHTML = `
                <span>${keyword}</span>
                <button class="remove-keyword" data-keyword="${keyword}">×</button>
            `;
            
            keywordsList.appendChild(keywordElement);
            keywordInput.value = '';
            console.log('Mot-clé ajouté:', keyword);
            console.log('Liste des mots-clés:', keywords);
        }
    }
    
    // Add keyword when button is clicked
    addKeywordBtn.addEventListener('click', function(e) {
        console.log('Bouton addKeyword cliqué');
        addKeyword();
    });
    
    // Add keyword when Enter is pressed
    keywordInput.addEventListener('keypress', function(e) {
        if (e.key === 'Enter') {
            console.log('Touche Enter pressée dans l\'input');
            e.preventDefault();
            addKeyword();
        }
    });
    
    // Remove keyword when × is clicked
    keywordsList.addEventListener('click', function(e) {
        if (e.target.classList.contains('remove-keyword')) {
            console.log('Bouton de suppression cliqué');
            const keyword = e.target.dataset.keyword;
            keywords = keywords.filter(k => k !== keyword);
            e.target.parentElement.remove();
            console.log('Mot-clé supprimé:', keyword);
            console.log('Liste des mots-clés mise à jour:', keywords);
        }
    });
    
    // Start collection - Ajout d'un event listener avec gestion des erreurs
    startCollectBtn.addEventListener('click', function(e) {
        console.log('Bouton de collecte cliqué!');
        
        try {
            const collectName = collectNameInput.value.trim();
            console.log('Nom de la collecte:', collectName);
            
            if (!collectName) {
                alert('Veuillez entrer un nom pour la collecte');
                console.warn('Nom de collecte manquant');
                return;
            }
            
            if (keywords.length === 0) {
                alert('Veuillez ajouter au moins un mot-clé');
                console.warn('Aucun mot-clé spécifié');
                return;
            }
            
            // Get selected networks
            const networks = [];
            if (document.getElementById('twitterCheck') && document.getElementById('twitterCheck').checked) {
                networks.push('twitter');
            }
            
            if (document.getElementById('blueskyCheck') && document.getElementById('blueskyCheck').checked) {
                networks.push('bluesky');
            }
            
            console.log('Réseaux sélectionnés:', networks);
            
            if (networks.length === 0) {
                alert('Veuillez sélectionner au moins un réseau social');
                console.warn('Aucun réseau social sélectionné');
                return;
            }
            
            // Disable button and show loading state
            startCollectBtn.disabled = true;
            startCollectBtn.textContent = 'Collecte en cours...';
            
            const requestUrl = `/projets/${projectId}/collect/start`;
            const requestBody = {
                name: collectName,
                keywords: keywords,
                networks: networks,
                limit: parseInt(document.getElementById('tweetLimit').value) || 10
            };
            
            console.log('URL de la requête:', requestUrl);
            console.log('Corps de la requête:', requestBody);
            
            // Send request to start collection
            fetch(requestUrl, {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json'
                },
                body: JSON.stringify(requestBody)
            })
            .then(response => {
                console.log('Réponse reçue:', response);
                if (!response.ok) {
                    throw new Error(`Erreur de serveur: ${response.status} ${response.statusText}`);
                }
                return response.json();
            })
            .then(data => {
                console.log('Données reçues:', data);
                if (data.success) {
                    alert(`Collecte terminée : ${data.message}`);
                    window.location.href = `/projets/${projectId}/collect`;
                } else {
                    alert(`Erreur: ${data.message}`);
                }
            })
            .catch(error => {
                console.error('Erreur lors de la requête:', error);
                alert(`Une erreur est survenue lors de la collecte: ${error.message}`);
            })
            .finally(() => {
                // Re-enable button
                startCollectBtn.disabled = false;
                startCollectBtn.textContent = 'Démarrer la collecte';
            });
        } catch (error) {
            console.error('Erreur dans le gestionnaire d\'événements:', error);
            alert(`Une erreur inattendue s'est produite: ${error.message}`);
            startCollectBtn.disabled = false;
            startCollectBtn.textContent = 'Démarrer la collecte';
        }
    });
    
    console.log('Initialisation terminée');
}

// Essayer deux méthodes d'initialisation pour s'assurer que le code s'exécute
document.addEventListener('DOMContentLoaded', initCollect);

// Si le DOM est déjà chargé, initialiser directement
if (document.readyState === 'interactive' || document.readyState === 'complete') {
    console.log('DOM déjà chargé, initialisation immédiate');
    setTimeout(initCollect, 100); // Petit délai pour s'assurer que tout est bien chargé
} 