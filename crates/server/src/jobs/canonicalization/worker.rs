use crate::canonicalization::{CanonicalizationConfig, CanonicalizationMode};
use crate::error::{PsmError, Result};
use crate::state::AppState;
use crate::storage::DeltaObject;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use tokio::time::interval;

use super::processor::process_candidates;

#[async_trait]
trait Processor: Send + Sync {
    async fn process_all_accounts(&self, state: &AppState) -> Result<()>;
    async fn process_account(&self, state: &AppState, account_id: &str) -> Result<()>;
}

struct DeltasProcessor {
    config: CanonicalizationConfig,
}

impl DeltasProcessor {
    fn new(config: CanonicalizationConfig) -> Self {
        Self { config }
    }

    fn filter_ready_candidates(&self, deltas: &[DeltaObject]) -> Vec<DeltaObject> {
        let now = Utc::now();
        let mut candidates: Vec<DeltaObject> = deltas
            .iter()
            .filter(|delta| self.is_ready_candidate(delta, &now))
            .cloned()
            .collect();

        candidates.sort_by_key(|d| d.nonce);
        candidates
    }

    fn is_ready_candidate(&self, delta: &DeltaObject, now: &DateTime<Utc>) -> bool {
        if !delta.status.is_candidate() {
            return false;
        }

        let candidate_at_str = delta.status.timestamp();
        if let Ok(candidate_at) = DateTime::parse_from_rfc3339(candidate_at_str) {
            let elapsed = now.signed_duration_since(candidate_at);
            return elapsed.num_seconds() >= self.config.delay_seconds as i64;
        }

        false
    }
}

#[async_trait]
impl Processor for DeltasProcessor {
    async fn process_all_accounts(&self, state: &AppState) -> Result<()> {
        let account_ids = state
            .metadata
            .list()
            .await
            .map_err(|e| PsmError::StorageError(format!("Failed to list accounts: {e}")))?;

        for account_id in account_ids {
            if let Err(e) = self.process_account(state, &account_id).await {
                eprintln!("Failed to process canonicalizations for account {account_id}: {e}");
            }
        }

        Ok(())
    }

    async fn process_account(&self, state: &AppState, account_id: &str) -> Result<()> {
        let account_metadata = state
            .metadata
            .get(account_id)
            .await
            .map_err(|e| PsmError::StorageError(format!("Failed to get metadata: {e}")))?
            .ok_or_else(|| PsmError::InvalidInput("Account metadata not found".to_string()))?;

        let storage_backend = state
            .storage
            .get(&account_metadata.storage_type)
            .map_err(PsmError::ConfigurationError)?;

        let all_deltas = storage_backend
            .pull_deltas_after(account_id, 0)
            .await
            .map_err(|e| PsmError::StorageError(format!("Failed to pull deltas: {e}")))?;

        let candidates = self.filter_ready_candidates(&all_deltas);
        process_candidates(state, &storage_backend, candidates, account_id).await?;

        Ok(())
    }
}

struct TestDeltasProcessor;

impl TestDeltasProcessor {
    fn filter_pending_candidates(&self, deltas: &[DeltaObject]) -> Vec<DeltaObject> {
        let mut candidates: Vec<DeltaObject> = deltas
            .iter()
            .filter(|delta| delta.status.is_candidate())
            .cloned()
            .collect();

        candidates.sort_by_key(|d| d.nonce);
        candidates
    }
}

#[async_trait]
impl Processor for TestDeltasProcessor {
    async fn process_all_accounts(&self, state: &AppState) -> Result<()> {
        let account_ids = state
            .metadata
            .list()
            .await
            .map_err(|e| PsmError::StorageError(format!("Failed to list accounts: {e}")))?;

        for account_id in account_ids {
            if let Err(e) = self.process_account(state, &account_id).await {
                eprintln!("Failed to process canonicalizations for account {account_id}: {e}");
            }
        }

        Ok(())
    }

    async fn process_account(&self, state: &AppState, account_id: &str) -> Result<()> {
        let account_metadata = state
            .metadata
            .get(account_id)
            .await
            .map_err(|e| PsmError::StorageError(format!("Failed to get metadata: {e}")))?
            .ok_or_else(|| PsmError::InvalidInput("Account metadata not found".to_string()))?;

        let storage_backend = state
            .storage
            .get(&account_metadata.storage_type)
            .map_err(PsmError::ConfigurationError)?;

        let all_deltas = storage_backend
            .pull_deltas_after(account_id, 0)
            .await
            .map_err(|e| PsmError::StorageError(format!("Failed to pull deltas: {e}")))?;

        let candidates = self.filter_pending_candidates(&all_deltas);
        process_candidates(state, &storage_backend, candidates, account_id).await?;

        Ok(())
    }
}

pub fn start_worker(state: AppState) {
    tokio::spawn(async move {
        run_worker(state).await;
    });
}

async fn run_worker(state: AppState) {
    let config = match &state.canonicalization_mode {
        CanonicalizationMode::Enabled(config) => config.clone(),
        CanonicalizationMode::Optimistic => {
            eprintln!(
                "Warning: Canonicalization worker started in Optimistic mode - this should not happen"
            );
            return;
        }
    };

    let processor = DeltasProcessor::new(config);
    let mut interval_timer = interval(processor.config.check_interval());

    loop {
        interval_timer.tick().await;

        if let Err(e) = processor.process_all_accounts(&state).await {
            eprintln!("Canonicalization worker error: {e}");
        }
    }
}

pub async fn process_all_accounts_now(state: &AppState) -> Result<()> {
    let processor = TestDeltasProcessor;
    processor.process_all_accounts(state).await
}
