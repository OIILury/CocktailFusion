pub mod export;
pub mod cleanup;
pub mod index_creation;
pub mod ingestion;
pub mod top_hashtags;
pub mod cooccurrence;
pub mod schema_copy;

use crate::routes::automation::AutomationContext;

/// Trait commun pour toutes les étapes de l'automatisation
#[async_trait::async_trait]
pub trait AutomationStep {
    /// Nom de l'étape
    fn name(&self) -> &'static str;
    
    /// Exécute l'étape
    async fn execute(&self, context: &AutomationContext) -> Result<(), Box<dyn std::error::Error>>;
    
    /// Vérifie si l'étape peut être exécutée en parallèle
    fn can_run_parallel(&self) -> bool {
        false
    }
} 