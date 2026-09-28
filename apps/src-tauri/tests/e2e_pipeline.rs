//! End-to-end pipeline test against the REAL stack: HTTP probe → SQLite task →
//! yt-dlp download → progress events → file on disk.
//!
//! Ignored by default (hits the network and spawns yt-dlp). Run explicitly:
//!   cargo test --test e2e_pipeline -- --ignored --nocapture
//! Requires `yt-dlp` on PATH. The default MP4 URL is a single progressive file,
//! so FFmpeg is not needed.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use video_dl::application::events::DomainEvent;
use video_dl::application::ports::{
    DownloadEngine, DownloadPlanner, DownloadRepository, EventPublisher, MediaProbe, TaskController,
};
use video_dl::application::use_cases::CancelDownload;
use video_dl::domain::entities::{Download, Task, TaskState};
use video_dl::domain::value_objects::{MediaFormat, MediaUrl};
use video_dl::infrastructure::database::SqlxDownloadRepository;
use video_dl::infrastructure::download::YtDlpEngine;
use video_dl::infrastructure::events::InMemoryEventBus;
use video_dl::infrastructure::network::HttpMediaProbe;
use video_dl::infrastructure::providers::{DownloadOptions, ProviderRegistry, RegistryPlanner};
use video_dl::infrastructure::scheduler::{RunningTasks, TaskScheduler};

const TEST_URL: &str = "https://download.samplelib.com/mp4/sample-5s.mp4";

#[tokio::test]
#[ignore = "hits the network and runs yt-dlp; run with --ignored"]
async fn real_download_pipeline_mp4() {
    let out_dir = std::env::temp_dir().join(format!("vdl-e2e-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&out_dir).unwrap();

    // 1. DETECTION — real HTTP probe.
    let probe = HttpMediaProbe::new();
    let url = MediaUrl::parse(TEST_URL).unwrap();
    let source = probe
        .probe(&url, None)
        .await
        .expect("probe should detect the mp4");
    assert_eq!(source.format, MediaFormat::ProgressiveMp4);
    println!(
        "[1] PROBE OK   → format={:?}, protected={}",
        source.format, source.protected
    );

    // 2. TASK — persisted in SQLite (in-memory).
    let repo: Arc<dyn DownloadRepository> =
        Arc::new(SqlxDownloadRepository::in_memory().await.unwrap());
    let download = Download::new(url.clone(), "generic", source.format);
    let task = Task::new(download.id);
    let task_id = task.id;
    repo.create(download, task).await.unwrap();
    println!("[2] SQLITE OK  → download + task persisted, state=Queued");

    // 3. SCHEDULER — real yt-dlp engine + provider registry planner.
    let engine: Arc<dyn DownloadEngine> = Arc::new(YtDlpEngine::new("yt-dlp", None));
    let planner: Arc<dyn DownloadPlanner> = Arc::new(RegistryPlanner::new(
        ProviderRegistry::with_defaults(),
        out_dir.to_string_lossy().to_string(),
        DownloadOptions::default(),
    ));
    let events = Arc::new(InMemoryEventBus::new(256));

    // Observe progress + state events on the bus.
    let mut rx = events.subscribe();
    let saw_progress = Arc::new(AtomicBool::new(false));
    let saw_running = Arc::new(AtomicBool::new(false));
    let progress_ticks = Arc::new(AtomicU8::new(0));
    let (p, r, t) = (saw_progress.clone(), saw_running.clone(), progress_ticks.clone());
    let watcher = tokio::spawn(async move {
        while let Ok(ev) = rx.recv().await {
            match ev {
                DomainEvent::TaskStateChanged { state, .. } => {
                    if state == TaskState::Running {
                        r.store(true, Ordering::SeqCst);
                    }
                    println!("    · state → {state}");
                }
                DomainEvent::ProgressUpdated { progress, .. } => {
                    p.store(true, Ordering::SeqCst);
                    let n = t.fetch_add(1, Ordering::SeqCst);
                    if n < 12 {
                        println!("    · progress {:.1}%", progress.percent);
                    }
                }
                _ => {}
            }
        }
    });

    let publisher: Arc<dyn EventPublisher> = events.clone();
    let scheduler =
        TaskScheduler::new(repo.clone(), engine, planner, publisher, 1, RunningTasks::new());

    // 4. RUN — spawn yt-dlp, stream progress, finalize.
    println!("[3] YT-DLP     → starting download…");
    scheduler
        .run_task(task_id)
        .await
        .expect("download should succeed");
    watcher.abort();

    // 5. ASSERTIONS — completed + file on disk.
    let final_task = repo.get_task(task_id).await.unwrap();
    assert_eq!(final_task.state, TaskState::Completed, "task must be Completed");
    assert!(saw_running.load(Ordering::SeqCst), "must have gone Running");
    assert!(saw_progress.load(Ordering::SeqCst), "must have emitted progress");

    let files: Vec<_> = std::fs::read_dir(&out_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    let total: u64 = files.iter().map(|f| f.metadata().unwrap().len()).sum();
    assert!(!files.is_empty() && total > 0, "a non-empty file must exist");

    println!(
        "[4] DONE       → state=Completed, {} file(s), {} bytes",
        files.len(),
        total
    );
    for f in &files {
        println!("               → {}", f.file_name().to_string_lossy());
    }

    // output_path captured + persisted on the download.
    let dl = repo.get_download(final_task.download_id).await.unwrap();
    println!("[5] OUTPUT     → {:?}", dl.output_path);
    assert!(dl.output_path.is_some(), "output_path should be captured");

    // history archived with byte count.
    let history = repo.recent_history(10).await.unwrap();
    assert_eq!(history.len(), 1, "one history entry expected");
    assert!(history[0].bytes.unwrap_or(0) > 0, "history should record bytes");
    println!(
        "[6] HISTORY    → {} entry, {} bytes",
        history.len(),
        history[0].bytes.unwrap_or(0)
    );

    std::fs::remove_dir_all(&out_dir).ok();
}

#[tokio::test]
#[ignore = "hits the network and runs yt-dlp; run with --ignored"]
async fn real_cancel_kills_ytdlp_mid_download() {
    let out_dir = std::env::temp_dir().join(format!("vdl-cancel-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&out_dir).unwrap();

    let url = MediaUrl::parse(TEST_URL).unwrap();
    let repo: Arc<dyn DownloadRepository> =
        Arc::new(SqlxDownloadRepository::in_memory().await.unwrap());
    let download = Download::new(url, "generic", MediaFormat::ProgressiveMp4);
    let task = Task::new(download.id);
    let task_id = task.id;
    repo.create(download, task).await.unwrap();

    // Throttle to ~30 KB/s so the 2.8 MB file would take ~95s — plenty of time
    // to cancel mid-download.
    let engine: Arc<dyn DownloadEngine> =
        Arc::new(YtDlpEngine::new("yt-dlp", None).with_rate_limit(Some(30_000)));
    let planner: Arc<dyn DownloadPlanner> = Arc::new(RegistryPlanner::new(
        ProviderRegistry::with_defaults(),
        out_dir.to_string_lossy().to_string(),
        DownloadOptions::default(),
    ));
    let events = Arc::new(InMemoryEventBus::new(64));
    let publisher: Arc<dyn EventPublisher> = events.clone();
    let running = RunningTasks::new();
    let controller: Arc<dyn TaskController> = Arc::new(running.clone());
    let scheduler = TaskScheduler::new(
        repo.clone(),
        engine,
        planner,
        publisher.clone(),
        1,
        running.clone(),
    );

    let started = Instant::now();
    let sched = scheduler.clone();
    let handle = tokio::spawn(async move { sched.run_task(task_id).await });

    // Wait until yt-dlp is actually running, then let it download for a bit.
    for _ in 0..300 {
        if running.is_running(task_id) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    tokio::time::sleep(Duration::from_millis(1500)).await;

    println!("[cancel] interrupting yt-dlp mid-download…");
    CancelDownload::new(repo.clone(), publisher, controller)
        .execute(task_id)
        .await
        .unwrap();

    // The process must die promptly — far below the ~95s full download time.
    let joined = tokio::time::timeout(Duration::from_secs(15), handle).await;
    assert!(joined.is_ok(), "run_task did not stop after cancel");
    let elapsed = started.elapsed();
    println!("[cancel] run_task returned after {:?}", elapsed);
    assert!(
        elapsed < Duration::from_secs(30),
        "should stop well before the throttled download would finish"
    );
    assert_eq!(
        repo.get_task(task_id).await.unwrap().state,
        TaskState::Cancelled
    );

    std::fs::remove_dir_all(&out_dir).ok();
}
