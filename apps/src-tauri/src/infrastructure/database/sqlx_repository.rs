use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteRow};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use crate::application::error::ApplicationError;
use crate::application::ports::DownloadRepository;
use crate::domain::entities::{Download, HistoryEntry, Task, TaskState};
use crate::domain::value_objects::{MediaFormat, MediaUrl, Progress};

/// SQLite-backed repository (SQLx). Swaps in for `InMemoryDownloadRepository`
/// behind the same port — use cases are unchanged.
pub struct SqlxDownloadRepository {
    pool: SqlitePool,
}

impl SqlxDownloadRepository {
    /// Open (creating if missing) a file-backed database with WAL and foreign
    /// keys, then apply migrations.
    pub async fn connect_file(path: &str, max_connections: u32) -> Result<Self, ApplicationError> {
        let opts = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            // Wait (instead of failing instantly with "database is locked") if
            // another connection/instance holds the DB — e.g. a leftover
            // process during a relaunch. Without this, a transient lock made
            // bootstrap panic and the app crashed on start.
            .busy_timeout(std::time::Duration::from_secs(15))
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(max_connections.max(1))
            .connect_with(opts)
            .await
            .map_err(repo_err)?;
        Self::migrate(&pool).await?;
        Ok(Self { pool })
    }

    /// In-memory database for tests. `max_connections(1)` keeps the single
    /// connection (and thus the data) alive for the pool's lifetime.
    pub async fn in_memory() -> Result<Self, ApplicationError> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .map_err(repo_err)?;
        Self::migrate(&pool).await?;
        Ok(Self { pool })
    }

    /// Clone of the underlying pool, so other repositories (e.g. settings) can
    /// share the same SQLite database.
    pub fn pool(&self) -> SqlitePool {
        self.pool.clone()
    }

    async fn migrate(pool: &SqlitePool) -> Result<(), ApplicationError> {
        sqlx::migrate!("./migrations")
            .run(pool)
            .await
            .map_err(|e| ApplicationError::Repository(format!("migration failed: {e}")))
    }
}

#[async_trait]
impl DownloadRepository for SqlxDownloadRepository {
    async fn create(&self, download: Download, task: Task) -> Result<(), ApplicationError> {
        let mut tx = self.pool.begin().await.map_err(repo_err)?;

        sqlx::query(
            "INSERT INTO downloads (id, source_url, provider_id, title, format, output_path, created_at, max_height, audio_only, referer)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(download.id.to_string())
        .bind(download.source_url.as_str())
        .bind(&download.provider_id)
        .bind(&download.title)
        .bind(format_to_str(download.format))
        .bind(&download.output_path)
        .bind(download.created_at.to_rfc3339())
        .bind(download.max_height.map(|h| h as i64))
        .bind(download.audio_only as i64)
        .bind(&download.referer)
        .execute(&mut *tx)
        .await
        .map_err(repo_err)?;

        insert_task(&mut tx, &task).await?;

        tx.commit().await.map_err(repo_err)
    }

    async fn get_download(&self, id: Uuid) -> Result<Download, ApplicationError> {
        let row = sqlx::query("SELECT * FROM downloads WHERE id = ?")
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await
            .map_err(repo_err)?;
        match row {
            Some(row) => map_download(&row),
            None => Err(ApplicationError::NotFound(format!("download {id}"))),
        }
    }

    async fn save_download(&self, download: Download) -> Result<(), ApplicationError> {
        sqlx::query("UPDATE downloads SET title = ?, output_path = ? WHERE id = ?")
            .bind(&download.title)
            .bind(&download.output_path)
            .bind(download.id.to_string())
            .execute(&self.pool)
            .await
            .map_err(repo_err)?;
        Ok(())
    }

    async fn get_task(&self, id: Uuid) -> Result<Task, ApplicationError> {
        let row = sqlx::query("SELECT * FROM tasks WHERE id = ?")
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await
            .map_err(repo_err)?;
        match row {
            Some(row) => map_task(&row),
            None => Err(ApplicationError::NotFound(format!("task {id}"))),
        }
    }

    async fn save_task(&self, task: Task) -> Result<(), ApplicationError> {
        let mut tx = self.pool.begin().await.map_err(repo_err)?;
        // Upsert so this works whether or not the task already exists.
        insert_task(&mut tx, &task).await?;
        tx.commit().await.map_err(repo_err)
    }

    async fn list_tasks_by_state(
        &self,
        state: TaskState,
    ) -> Result<Vec<Task>, ApplicationError> {
        let rows = sqlx::query("SELECT * FROM tasks WHERE state = ? ORDER BY created_at")
            .bind(state.to_string())
            .fetch_all(&self.pool)
            .await
            .map_err(repo_err)?;
        rows.iter().map(map_task).collect()
    }

    async fn list_all_tasks(&self) -> Result<Vec<Task>, ApplicationError> {
        let rows = sqlx::query("SELECT * FROM tasks ORDER BY created_at")
            .fetch_all(&self.pool)
            .await
            .map_err(repo_err)?;
        rows.iter().map(map_task).collect()
    }

    async fn list_active(&self) -> Result<Vec<(Download, Task)>, ApplicationError> {
        let rows = sqlx::query(
            "SELECT * FROM tasks
             WHERE state NOT IN ('completed', 'cancelled')
             ORDER BY created_at",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(repo_err)?;

        let mut out = Vec::with_capacity(rows.len());
        for row in &rows {
            let task = map_task(row)?;
            let download = self.get_download(task.download_id).await?;
            out.push((download, task));
        }
        Ok(out)
    }

    async fn record_history(
        &self,
        download_id: Uuid,
        final_path: Option<String>,
        bytes: Option<u64>,
    ) -> Result<(), ApplicationError> {
        sqlx::query(
            "INSERT INTO history (id, download_id, completed_at, final_path, bytes)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(download_id.to_string())
        .bind(Utc::now().to_rfc3339())
        .bind(final_path)
        .bind(bytes.map(|b| b as i64))
        .execute(&self.pool)
        .await
        .map_err(repo_err)?;
        Ok(())
    }

    async fn recent_history(&self, limit: u32) -> Result<Vec<HistoryEntry>, ApplicationError> {
        let rows = sqlx::query(
            "SELECT h.download_id AS download_id, h.completed_at AS completed_at,
                    h.final_path AS final_path, h.bytes AS bytes,
                    d.title AS title, d.source_url AS source_url
             FROM history h
             JOIN downloads d ON d.id = h.download_id
             ORDER BY h.completed_at DESC
             LIMIT ?",
        )
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(repo_err)?;

        rows.iter().map(map_history).collect()
    }

    async fn delete_download(&self, id: Uuid) -> Result<(), ApplicationError> {
        // Cascades to `tasks` and `queue` (ON DELETE CASCADE). Never called
        // on a completed download (those have a history row referencing
        // this id), so `recent_history`'s JOIN never dangles.
        sqlx::query("DELETE FROM downloads WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await
            .map_err(repo_err)?;
        Ok(())
    }

    async fn delete_history_entry(&self, download_id: Uuid) -> Result<(), ApplicationError> {
        sqlx::query("DELETE FROM history WHERE download_id = ?")
            .bind(download_id.to_string())
            .execute(&self.pool)
            .await
            .map_err(repo_err)?;
        Ok(())
    }
}

// ---- helpers -----------------------------------------------------------------

async fn insert_task(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    task: &Task,
) -> Result<(), ApplicationError> {
    sqlx::query(
        "INSERT INTO tasks
            (id, download_id, state, progress_percent, speed_bps, eta_secs,
             error_code, error_message, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET
            state            = excluded.state,
            progress_percent = excluded.progress_percent,
            speed_bps        = excluded.speed_bps,
            eta_secs         = excluded.eta_secs,
            error_code       = excluded.error_code,
            error_message    = excluded.error_message,
            updated_at       = excluded.updated_at",
    )
    .bind(task.id.to_string())
    .bind(task.download_id.to_string())
    .bind(task.state.to_string())
    .bind(task.progress.percent as f64)
    .bind(task.progress.speed_bps.map(|v| v as i64))
    .bind(task.progress.eta_secs.map(|v| v as i64))
    .bind(&task.error_code)
    .bind(&task.error_message)
    .bind(task.created_at.to_rfc3339())
    .bind(task.updated_at.to_rfc3339())
    .execute(&mut **tx)
    .await
    .map_err(repo_err)
    .map(|_| ())
}

fn repo_err(e: sqlx::Error) -> ApplicationError {
    ApplicationError::Repository(e.to_string())
}

fn map_download(row: &SqliteRow) -> Result<Download, ApplicationError> {
    let id: String = row.try_get("id").map_err(repo_err)?;
    let source_url: String = row.try_get("source_url").map_err(repo_err)?;
    let provider_id: String = row.try_get("provider_id").map_err(repo_err)?;
    let title: Option<String> = row.try_get("title").map_err(repo_err)?;
    let format: String = row.try_get("format").map_err(repo_err)?;
    let output_path: Option<String> = row.try_get("output_path").map_err(repo_err)?;
    let created_at: String = row.try_get("created_at").map_err(repo_err)?;
    let max_height: Option<i64> = row.try_get("max_height").map_err(repo_err)?;
    let audio_only: i64 = row.try_get("audio_only").map_err(repo_err)?;
    let referer: Option<String> = row.try_get("referer").map_err(repo_err)?;

    Ok(Download {
        id: parse_uuid(&id)?,
        source_url: MediaUrl::parse(source_url)?,
        provider_id,
        title,
        format: format_from_str(&format)?,
        output_path,
        created_at: parse_dt(&created_at)?,
        max_height: max_height.map(|h| h as u32),
        audio_only: audio_only != 0,
        referer,
    })
}

fn map_task(row: &SqliteRow) -> Result<Task, ApplicationError> {
    let id: String = row.try_get("id").map_err(repo_err)?;
    let download_id: String = row.try_get("download_id").map_err(repo_err)?;
    let state: String = row.try_get("state").map_err(repo_err)?;
    let percent: f64 = row.try_get("progress_percent").map_err(repo_err)?;
    let speed_bps: Option<i64> = row.try_get("speed_bps").map_err(repo_err)?;
    let eta_secs: Option<i64> = row.try_get("eta_secs").map_err(repo_err)?;
    let error_code: Option<String> = row.try_get("error_code").map_err(repo_err)?;
    let error_message: Option<String> = row.try_get("error_message").map_err(repo_err)?;
    let created_at: String = row.try_get("created_at").map_err(repo_err)?;
    let updated_at: String = row.try_get("updated_at").map_err(repo_err)?;

    Ok(Task {
        id: parse_uuid(&id)?,
        download_id: parse_uuid(&download_id)?,
        state: state_from_str(&state)?,
        progress: Progress {
            percent: percent as f32,
            speed_bps: speed_bps.map(|v| v as u64),
            eta_secs: eta_secs.map(|v| v as u64),
        },
        error_code,
        error_message,
        created_at: parse_dt(&created_at)?,
        updated_at: parse_dt(&updated_at)?,
    })
}

fn map_history(row: &SqliteRow) -> Result<HistoryEntry, ApplicationError> {
    let download_id: String = row.try_get("download_id").map_err(repo_err)?;
    let title: Option<String> = row.try_get("title").map_err(repo_err)?;
    let source_url: String = row.try_get("source_url").map_err(repo_err)?;
    let final_path: Option<String> = row.try_get("final_path").map_err(repo_err)?;
    let bytes: Option<i64> = row.try_get("bytes").map_err(repo_err)?;
    let completed_at: String = row.try_get("completed_at").map_err(repo_err)?;

    Ok(HistoryEntry {
        download_id: parse_uuid(&download_id)?,
        title,
        source_url,
        final_path,
        bytes: bytes.map(|b| b as u64),
        completed_at: parse_dt(&completed_at)?,
    })
}

fn parse_uuid(s: &str) -> Result<Uuid, ApplicationError> {
    Uuid::parse_str(s).map_err(|e| ApplicationError::Repository(format!("bad uuid {s}: {e}")))
}

fn parse_dt(s: &str) -> Result<DateTime<Utc>, ApplicationError> {
    DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&Utc))
        .map_err(|e| ApplicationError::Repository(format!("bad datetime {s}: {e}")))
}

fn format_to_str(f: MediaFormat) -> &'static str {
    match f {
        MediaFormat::ProgressiveMp4 => "progressive_mp4",
        MediaFormat::WebM => "webm",
        MediaFormat::Mov => "mov",
        MediaFormat::Hls => "hls",
        MediaFormat::Dash => "dash",
    }
}

fn format_from_str(s: &str) -> Result<MediaFormat, ApplicationError> {
    match s {
        "progressive_mp4" => Ok(MediaFormat::ProgressiveMp4),
        "webm" => Ok(MediaFormat::WebM),
        "mov" => Ok(MediaFormat::Mov),
        "hls" => Ok(MediaFormat::Hls),
        "dash" => Ok(MediaFormat::Dash),
        other => Err(ApplicationError::Repository(format!("unknown format: {other}"))),
    }
}

fn state_from_str(s: &str) -> Result<TaskState, ApplicationError> {
    match s {
        "queued" => Ok(TaskState::Queued),
        "running" => Ok(TaskState::Running),
        "paused" => Ok(TaskState::Paused),
        "completed" => Ok(TaskState::Completed),
        "failed" => Ok(TaskState::Failed),
        "cancelled" => Ok(TaskState::Cancelled),
        other => Err(ApplicationError::Repository(format!("unknown state: {other}"))),
    }
}
