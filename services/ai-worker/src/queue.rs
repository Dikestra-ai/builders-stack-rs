use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, watch};

// ── Job types ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummarizeJob {
    pub post_id: String,
    pub text: String,
}

#[derive(Debug, Clone)]
pub enum Job {
    Summarize(SummarizeJob),
}

// ── Queue ──────────────────────────────────────────────────────────────────────

pub struct Queue {
    sender: mpsc::Sender<Job>,
}

impl Queue {
    /// Create a new queue. Returns the `Queue` handle (for producers) and the
    /// `Receiver` end (passed to `run_queue`).
    pub fn new() -> (Self, mpsc::Receiver<Job>) {
        let (sender, receiver) = mpsc::channel(100);
        (Self { sender }, receiver)
    }

    /// Enqueue a job. Returns an error only if the receiver has been dropped.
    pub async fn add(&self, job: Job) -> Result<()> {
        self.sender
            .send(job)
            .await
            .map_err(|e| anyhow::anyhow!("Queue send error: {}", e))
    }
}

// ── Queue runner ───────────────────────────────────────────────────────────────

/// Drain `receiver` one job at a time. On failure retry up to `max_retries`
/// times with exponential back-off (`100 ms * attempt`). When `shutdown`
/// becomes `true`, drain remaining jobs in the channel then return.
pub async fn run_queue(
    mut receiver: mpsc::Receiver<Job>,
    max_retries: u32,
    mut shutdown: watch::Receiver<bool>,
) {
    loop {
        tokio::select! {
            biased;

            // Shutdown signal takes priority over new jobs.
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    tracing::info!("[ai-worker] draining remaining jobs");
                    // Drain whatever is still buffered in the channel.
                    while let Ok(job) = receiver.try_recv() {
                        process_with_retry(job, max_retries).await;
                    }
                    break;
                }
            }

            Some(job) = receiver.recv() => {
                process_with_retry(job, max_retries).await;
            }

            else => break,
        }
    }
}

// ── Retry wrapper ──────────────────────────────────────────────────────────────

async fn process_with_retry(job: Job, max_retries: u32) {
    for attempt in 0..=max_retries {
        match process_job(&job).await {
            Ok(()) => return,
            Err(e) if attempt < max_retries => {
                // Exponential back-off: 100 ms * attempt (1-based)
                let backoff =
                    std::time::Duration::from_millis(100 * (attempt + 1) as u64);
                tracing::warn!(
                    "Job failed (attempt {}/{}): {}. Retrying in {:?}",
                    attempt + 1,
                    max_retries + 1,
                    e,
                    backoff
                );
                tokio::time::sleep(backoff).await;
            }
            Err(e) => {
                tracing::error!("Job failed after {} attempts: {}", max_retries + 1, e);
            }
        }
    }
}

// ── Job handlers ───────────────────────────────────────────────────────────────

async fn process_job(job: &Job) -> Result<()> {
    match job {
        Job::Summarize(s) => handle_summarize(s).await,
    }
}

async fn handle_summarize(job: &SummarizeJob) -> Result<()> {
    tracing::info!(post_id = %job.post_id, "summarizing post");

    let result = stack_ai::default_client()
        .generate(stack_ai::GenerateOptions {
            system: Some("You summarize text in one sentence.".to_string()),
            prompt: Some(job.text.clone()),
            max_tokens: Some(100),
            ..Default::default()
        })
        .await?;

    tracing::info!(
        post_id = %job.post_id,
        summary = %result.text,
        prompt_tokens = result.prompt_tokens,
        completion_tokens = result.completion_tokens,
        "summarization complete",
    );

    // TODO: persist summary via stack_db
    // stack_db::update_post_summary(&pool, &job.post_id, &result.text).await?;

    Ok(())
}
