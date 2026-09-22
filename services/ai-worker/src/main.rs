mod queue;

use queue::{Job, Queue, SummarizeJob};
use stack_observability::init_tracing;
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::watch;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    tracing::info!("[ai-worker] started");

    let (queue, receiver) = Queue::new();
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    // Demo job: enqueue unless AI_WORKER_DEMO=0
    if std::env::var("AI_WORKER_DEMO").as_deref() != Ok("0") {
        queue
            .add(Job::Summarize(SummarizeJob {
                post_id: "p_demo".to_string(),
                text: "The builders-stack is a reference monorepo for shipping fast.".to_string(),
            }))
            .await?;
        tracing::info!("[ai-worker] demo job enqueued");
    }

    let worker_handle = tokio::spawn(queue::run_queue(receiver, 2, shutdown_rx));

    // Wait for SIGINT or SIGTERM
    let mut sigint = signal(SignalKind::interrupt())?;
    let mut sigterm = signal(SignalKind::terminate())?;

    tokio::select! {
        _ = sigint.recv() => {
            tracing::info!("[ai-worker] SIGINT received — draining, then exit");
        }
        _ = sigterm.recv() => {
            tracing::info!("[ai-worker] SIGTERM received — draining, then exit");
        }
    }

    // Signal the queue runner to drain and exit
    let _ = shutdown_tx.send(true);

    // Give the in-flight job a moment (200 ms), then exit
    tokio::time::timeout(
        std::time::Duration::from_millis(200),
        worker_handle,
    )
    .await
    .ok();

    tracing::info!("[ai-worker] stopped");
    Ok(())
}
