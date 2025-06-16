document.addEventListener('DOMContentLoaded', function() {
    const fileInput = document.getElementById('fileInput');
    const fileDropZone = document.getElementById('fileDropZone');
    const selectedFiles = document.getElementById('selectedFiles');
    const startImport = document.getElementById('startImport');
    const previewContainer = document.getElementById('previewContainer');
    const twitterCheck = document.getElementById('twitterCheck');
    const blueskyCheck = document.getElementById('blueskyCheck');
    const singleFileMode = document.getElementById('singleFileMode');
    const multipleFilesMode = document.getElementById('multipleFilesMode');
    const singleFileCard = document.getElementById('singleFileCard');
    const multipleFilesCard = document.getElementById('multipleFilesCard');
    const dropZoneText = document.getElementById('dropZoneText');
    const importNameInput = document.getElementById('importName');
    let currentFiles = [];
    let currentSchemaName = null;
    let isImporting = false;
    let importTimeout = null;
    let isAnalyzing = false;
    let currentAnalysis = null;

    // Gestion du mode d'importation
    singleFileCard.addEventListener('click', () => {
        if (isAnalyzing || isImporting) return;
        
        singleFileMode.checked = true;
        dropZoneText.textContent = 'Glissez et déposez votre fichier CSV ici';
        fileInput.removeAttribute('multiple');
        currentFiles = [];
        selectedFiles.innerHTML = '';
        previewContainer.innerHTML = '';
        updateCardStyles();
    });

    multipleFilesCard.addEventListener('click', () => {
        if (isAnalyzing || isImporting) return;
        
        multipleFilesMode.checked = true;
        dropZoneText.textContent = 'Glissez et déposez vos fichiers CSV ici';
        fileInput.setAttribute('multiple', '');
        currentFiles = [];
        selectedFiles.innerHTML = '';
        previewContainer.innerHTML = '';
        updateCardStyles();
    });

    function updateCardStyles() {
        singleFileCard.style.borderColor = singleFileMode.checked ? '#e63429' : '#ddd';
        multipleFilesCard.style.borderColor = multipleFilesMode.checked ? '#e63429' : '#ddd';
    }

    // Initialiser les styles des cartes
    updateCardStyles();

    // Gestion du drag and drop
    fileDropZone.addEventListener('dragover', (e) => {
        if (isAnalyzing || isImporting) return;
        
        e.preventDefault();
        e.stopPropagation();
        fileDropZone.classList.add('dragover');
    });

    fileDropZone.addEventListener('dragleave', (e) => {
        if (isAnalyzing || isImporting) return;
        
        e.preventDefault();
        e.stopPropagation();
        fileDropZone.classList.remove('dragover');
    });

    fileDropZone.addEventListener('drop', (e) => {
        if (isAnalyzing || isImporting) return;
        
        e.preventDefault();
        e.stopPropagation();
        fileDropZone.classList.remove('dragover');
        const files = Array.from(e.dataTransfer.files);
        handleFiles(files);
    });

    // Gestion du clic sur le bouton "Parcourir"
    let lastFileInputChange = 0;
    fileInput.addEventListener('change', (e) => {
        // Empêcher les doubles déclenchements dans un court laps de temps
        const now = Date.now();
        if (now - lastFileInputChange < 500) {
            return;
        }
        lastFileInputChange = now;

        if (isAnalyzing || isImporting) return;
        
        const files = Array.from(e.target.files);
        handleFiles(files);
    });

    // Gestion des fichiers sélectionnés
    function handleFiles(files) {
        if (isAnalyzing || isImporting) return;
        
        const validFiles = files.filter(file => 
            file.type === 'text/csv' || 
            file.type === 'application/vnd.ms-excel' ||
            file.name.toLowerCase().endsWith('.csv')
        );

        if (validFiles.length === 0) {
            showNotification('error', 'Erreur', 'Veuillez sélectionner au moins un fichier CSV valide');
            return;
        }

        if (singleFileMode.checked && validFiles.length > 1) {
            showNotification('error', 'Erreur', 'Veuillez sélectionner un seul fichier en mode fichier unique');
            return;
        }

        // Vérifier la taille des fichiers
        const maxFileSize = 100 * 1024 * 1024; // 100 MB
        const oversizedFiles = validFiles.filter(file => file.size > maxFileSize);
        if (oversizedFiles.length > 0) {
            showNotification('warning', 'Attention', 
                `Les fichiers suivants dépassent la taille maximale de 100 MB : ${oversizedFiles.map(f => f.name).join(', ')}`);
            return;
        }

        currentFiles = validFiles;
        
        // Mettre à jour le nom de l'importation si en mode fichier unique
        if (singleFileMode.checked && validFiles.length === 1) {
            const fileNameWithoutExtension = validFiles[0].name.replace(/\.csv$/i, '');
            importNameInput.value = fileNameWithoutExtension;
        }
        
        // Afficher les fichiers sélectionnés
        selectedFiles.innerHTML = validFiles.map(file => `
            <div class="selected-file">
                <span class="file-name">${file.name}</span>
                <span class="file-size">${formatFileSize(file.size)}</span>
                <button class="remove-file" data-filename="${file.name}">×</button>
            </div>
        `).join('');

        // Ajouter les gestionnaires d'événements pour les boutons de suppression
        document.querySelectorAll('.remove-file').forEach(button => {
            button.addEventListener('click', (e) => {
                if (isAnalyzing || isImporting) return;
                
                const fileName = e.target.dataset.filename;
                currentFiles = currentFiles.filter(file => file.name !== fileName);
                if (currentFiles.length === 0) {
                    importNameInput.value = '';
                    previewContainer.innerHTML = '';
                    currentSchemaName = null;
                }
                handleFiles(currentFiles);
            });
        });

        // Analyser le premier fichier
        if (validFiles.length > 0) {
            analyzeFile(validFiles[0]);
        }
    }

    // Analyse du fichier CSV
    async function analyzeFile(file) {
        if (isAnalyzing) {
            return;
        }

        isAnalyzing = true;
        const formData = new FormData();
        formData.append('files', file);
        formData.append('mode', singleFileMode.checked ? 'single' : 'multiple');

        try {
            const notification = showNotification('info', 'Analyse en cours', 'Analyse du fichier CSV...', 0, true);
            
            const response = await fetch('/api/import/csv/analyze', {
                method: 'POST',
                body: formData
            });

            if (!response.ok) {
                const errorData = await response.json();
                throw new Error(errorData.message || 'Erreur lors de l\'analyse du fichier');
            }

            const data = await response.json();
            currentSchemaName = data.schema_name;
            currentAnalysis = data.analysis;
            displayPreview(data.analysis);
            
            updateProgressNotification(notification, 100, 'Analyse terminée');
            setTimeout(() => {
                notification.classList.remove('show');
                setTimeout(() => {
                    if (notification.parentNode) {
                        notification.parentNode.removeChild(notification);
                    }
                }, 300);
            }, 1000);

            showNotification('success', 'Analyse terminée', 'Le fichier a été analysé avec succès');
        } catch (error) {
            console.error('Erreur lors de l\'analyse:', error);
            showNotification('error', 'Erreur', error.message || 'Une erreur est survenue lors de l\'analyse du fichier');
            previewContainer.innerHTML = '';
            currentSchemaName = null;
            currentAnalysis = null;
        } finally {
            isAnalyzing = false;
        }
    }

    // Affichage de la prévisualisation
    function displayPreview(analysis) {
        const previewHtml = `
            <div class="preview-section">
                <h4>Prévisualisation des données</h4>
                <div class="preview-info">
                    <p>Nombre total de lignes: ${analysis.total_rows}</p>
                    <p>Nombre de colonnes: ${analysis.total_columns}</p>
                    <p>Encodage détecté: ${analysis.encoding}</p>
                    <p>Séparateur détecté: ${analysis.delimiter}</p>
                </div>
                
                <div class="preview-table-container">
                    <table class="preview-table">
                        <thead>
                            <tr>
                                ${analysis.headers.map(header => `<th>${header}</th>`).join('')}
                            </tr>
                        </thead>
                        <tbody>
                            ${analysis.preview.map(row => `
                                <tr>
                                    ${analysis.headers.map(header => `<td>${row[header] || ''}</td>`).join('')}
                                </tr>
                            `).join('')}
                        </tbody>
                    </table>
                </div>

                ${analysis.potential_issues.length > 0 ? `
                    <div class="preview-issues">
                        <h5>Problèmes potentiels détectés:</h5>
                        <ul>
                            ${analysis.potential_issues.map(issue => `
                                <li class="issue-item ${issue.issue_type.toLowerCase()}">
                                    ${issue.message}
                                    ${issue.affected_rows.length > 0 ? 
                                        ` (Lignes: ${issue.affected_rows.join(', ')})` : 
                                        ''}
                                </li>
                            `).join('')}
                        </ul>
                    </div>
                ` : ''}
            </div>
        `;

        previewContainer.innerHTML = previewHtml;
    }

    // Gestion de l'importation
    startImport.addEventListener('click', async () => {
        if (isImporting) {
            showNotification('warning', 'Attention', 'Une importation est déjà en cours');
            return;
        }

        if (currentFiles.length === 0) {
            showNotification('error', 'Erreur', 'Veuillez sélectionner au moins un fichier CSV');
            return;
        }

        if (!currentSchemaName || !currentAnalysis) {
            showNotification('error', 'Erreur', 'Veuillez d\'abord analyser un fichier CSV');
            return;
        }

        if (!twitterCheck.checked && !blueskyCheck.checked) {
            showNotification('warning', 'Attention', 'Veuillez sélectionner une source (Twitter ou Bluesky)');
            return;
        }

        const importName = importNameInput.value.trim();
        if (!importName) {
            showNotification('error', 'Erreur', 'Veuillez entrer un nom pour l\'importation');
            return;
        }

        // Désactiver le bouton et marquer l'importation comme en cours
        isImporting = true;
        startImport.disabled = true;
        startImport.textContent = 'Importation en cours...';

        // Définir un timeout de 30 minutes
        importTimeout = setTimeout(() => {
            if (isImporting) {
                isImporting = false;
                startImport.disabled = false;
                startImport.textContent = 'Démarrer l\'importation';
                showNotification('error', 'Timeout', 'L\'importation a dépassé le temps maximum autorisé (30 minutes)');
            }
        }, 30 * 60 * 1000);

        const source = twitterCheck.checked ? 'twitter' : 'bluesky';
        const formData = new FormData();
        
        // Ajouter tous les fichiers sélectionnés
        currentFiles.forEach(file => {
            formData.append('files', file);
        });

        formData.append('mode', singleFileMode.checked ? 'single' : 'multiple');
        formData.append('schema_name', currentSchemaName);
        formData.append('source', source);
        formData.append('name', importName);
        formData.append('analysis', JSON.stringify(currentAnalysis));

        try {
            const notification = showNotification('info', 'Importation en cours', 'Importation des données...', 0, true);
            
            // Utiliser XMLHttpRequest pour avoir une meilleure gestion de la progression
            const xhr = new XMLHttpRequest();
            xhr.open('POST', '/api/import/csv', true);

            // Gérer la progression de l'upload
            xhr.upload.onprogress = (event) => {
                if (event.lengthComputable) {
                    const progress = Math.round((event.loaded / event.total) * 100);
                    updateProgressNotification(notification, progress, `Importation en cours (${progress}%)`);
                }
            };

            // Gérer la réponse
            xhr.onload = function() {
                clearTimeout(importTimeout);
                isImporting = false;
                startImport.disabled = false;
                startImport.textContent = 'Démarrer l\'importation';

                if (xhr.status === 200) {
                    const result = JSON.parse(xhr.responseText);
                    updateProgressNotification(notification, 100, 'Importation terminée');
                    setTimeout(() => {
                        notification.classList.remove('show');
                        setTimeout(() => {
                            if (notification.parentNode) {
                                notification.parentNode.removeChild(notification);
                            }
                        }, 300);
                    }, 1000);

                    showNotification('success', 'Importation réussie', 
                        `${result.rows_imported} lignes ont été importées avec succès.${result.errors.length > 0 ? 
                            `\n${result.errors.length} erreurs ont été rencontrées.` : ''}`);
                    
                    // Réinitialiser l'interface
                    selectedFiles.innerHTML = '';
                    previewContainer.innerHTML = '';
                    currentFiles = [];
                    currentSchemaName = null;
                    currentAnalysis = null;
                    importNameInput.value = '';
                    twitterCheck.checked = false;
                    blueskyCheck.checked = false;
                } else {
                    const result = JSON.parse(xhr.responseText);
                    updateProgressNotification(notification, 0, 'Erreur lors de l\'importation');
                    showNotification('error', 'Erreur', result.message || 'Une erreur est survenue lors de l\'importation');
                }
            };

            xhr.onerror = function() {
                clearTimeout(importTimeout);
                isImporting = false;
                startImport.disabled = false;
                startImport.textContent = 'Démarrer l\'importation';
                updateProgressNotification(notification, 0, 'Erreur lors de l\'importation');
                showNotification('error', 'Erreur', 'Une erreur est survenue lors de l\'importation');
            };

            xhr.send(formData);
        } catch (error) {
            clearTimeout(importTimeout);
            isImporting = false;
            startImport.disabled = false;
            startImport.textContent = 'Démarrer l\'importation';
            console.error('Erreur lors de l\'importation:', error);
            showNotification('error', 'Erreur', 'Une erreur est survenue lors de l\'importation');
        }
    });

    // Gestion des cases à cocher des sources
    twitterCheck.addEventListener('change', function() {
        if (this.checked) {
            blueskyCheck.checked = false;
        }
    });

    blueskyCheck.addEventListener('change', function() {
        if (this.checked) {
            twitterCheck.checked = false;
        }
    });

    // Fonctions utilitaires
    function formatFileSize(bytes) {
        if (bytes === 0) return '0 Bytes';
        const k = 1024;
        const sizes = ['Bytes', 'KB', 'MB', 'GB'];
        const i = Math.floor(Math.log(bytes) / Math.log(k));
        return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
    }

    function showNotification(type, title, message, progress = null, persistent = false) {
        const container = document.getElementById('notificationContainer');
        const notification = document.createElement('div');
        notification.className = `notification ${type}`;
        
        let icon = '';
        switch(type) {
            case 'success':
                icon = '✅';
                break;
            case 'error':
                icon = '❌';
                break;
            case 'info':
                icon = 'ℹ️';
                break;
            case 'warning':
                icon = '⚠️';
                break;
        }

        notification.innerHTML = `
            <span class="notification-icon">${icon}</span>
            <div class="notification-content">
                <div class="notification-title">${title}</div>
                <div class="notification-message">${message}</div>
                ${progress !== null ? `
                    <div class="notification-progress">
                        <div class="notification-progress-bar" style="width: ${progress}%"></div>
                    </div>
                    <div class="notification-status">${progress}%</div>
                ` : ''}
            </div>
            <span class="notification-close">✕</span>
        `;

        container.appendChild(notification);
        
        // Animation d'entrée
        setTimeout(() => {
            notification.classList.add('show');
        }, 100);

        // Gestion de la fermeture manuelle
        const closeButton = notification.querySelector('.notification-close');
        if (closeButton) {
            closeButton.addEventListener('click', () => {
                closeNotification(notification);
            });
        }

        // Suppression automatique après 8 secondes si non persistante
        if (!persistent) {
            setTimeout(() => {
                closeNotification(notification);
            }, 8000);
        }

        return notification;
    }

    function closeNotification(notification) {
        notification.classList.remove('show');
        setTimeout(() => {
            if (notification.parentNode) {
                notification.parentNode.removeChild(notification);
            }
        }, 300);
    }

    function updateProgressNotification(notification, progress, message) {
        if (!notification || !notification.parentNode) return;
        
        const progressBar = notification.querySelector('.notification-progress-bar');
        const statusText = notification.querySelector('.notification-status');
        const messageElement = notification.querySelector('.notification-message');
        
        if (progressBar) progressBar.style.width = `${progress}%`;
        if (statusText) statusText.textContent = `${progress}%`;
        if (messageElement) messageElement.textContent = message;

        // Si la progression est à 100%, fermer la notification après un délai
        if (progress === 100) {
            setTimeout(() => {
                closeNotification(notification);
            }, 2000);
        }
    }
}); 