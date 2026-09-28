//! Reusable command handlers, one per user intention. Tauri-free so they can be
//! unit-tested directly; the `#[tauri::command]` wrappers (added with the app
//! shell) will be thin adapters calling these.

use std::time::Duration;

use uuid::Uuid;

use crate::application::use_cases::{ExtractFromPage, StartDownloadRequest};
use crate::state::AppState;

use super::dto::{
    DownloadViewDto, HistoryEntryDto, MediaSourceDto, SettingsDto, StartDownloadArgs,
    StartDownloadResponse,
};
use super::error::{IpcError, IpcErrorKind};

pub async fn probe_url(state: &AppState, url: String) -> Result<MediaSourceDto, IpcError> {
    let source = state.probe_url.execute(&url).await?;
    Ok(MediaSourceDto::from(source))
}

pub async fn start_download(
    state: &AppState,
    args: StartDownloadArgs,
) -> Result<StartDownloadResponse, IpcError> {
    let res = state
        .start_download
        .execute(StartDownloadRequest {
            url: args.url,
            provider_id: args.provider_id,
            format: args.format.into(),
            title: args.title,
            max_height: args.max_height,
            audio_only: args.audio_only,
            referer: args.referer,
        })
        .await?;
    Ok(StartDownloadResponse {
        download_id: res.download_id.to_string(),
        task_id: res.task_id.to_string(),
    })
}

pub async fn cancel_download(state: &AppState, task_id: String) -> Result<(), IpcError> {
    let id = parse_uuid(&task_id)?;
    state.cancel_download.execute(id).await?;
    Ok(())
}

pub async fn pause_download(state: &AppState, task_id: String) -> Result<(), IpcError> {
    let id = parse_uuid(&task_id)?;
    state.pause_resume.pause(id).await?;
    Ok(())
}

pub async fn resume_download(state: &AppState, task_id: String) -> Result<(), IpcError> {
    let id = parse_uuid(&task_id)?;
    state.pause_resume.resume(id).await?;
    Ok(())
}

pub async fn list_history(state: &AppState) -> Result<Vec<HistoryEntryDto>, IpcError> {
    let entries = state.list_history.execute(200).await?;
    Ok(entries.into_iter().map(HistoryEntryDto::from).collect())
}

pub async fn remove_download(state: &AppState, task_id: String) -> Result<(), IpcError> {
    let id = parse_uuid(&task_id)?;
    state.remove_download.execute(id).await?;
    Ok(())
}

pub async fn retry_download(state: &AppState, task_id: String) -> Result<(), IpcError> {
    let id = parse_uuid(&task_id)?;
    state.retry_download.execute(id).await?;
    Ok(())
}

pub async fn delete_history_entry(state: &AppState, download_id: String) -> Result<(), IpcError> {
    let id = parse_uuid(&download_id)?;
    state.delete_history_entry.execute(id).await?;
    Ok(())
}

pub async fn list_active(state: &AppState) -> Result<Vec<DownloadViewDto>, IpcError> {
    let views = state.list_active.execute().await?;
    Ok(views.into_iter().map(DownloadViewDto::from).collect())
}

pub async fn get_settings(state: &AppState) -> Result<SettingsDto, IpcError> {
    Ok(SettingsDto::from(state.get_settings.execute().await?))
}

pub async fn update_settings(state: &AppState, settings: SettingsDto) -> Result<(), IpcError> {
    state.update_settings.execute(settings.into()).await?;
    Ok(())
}

/// How long the hidden extraction window is given to observe network
/// requests before we give up and fall back to the interactive path.
const SILENT_EXTRACTION_TIMEOUT: Duration = Duration::from_secs(12);

pub async fn extract_from_page(
    extractor: &ExtractFromPage,
    url: String,
) -> Result<Vec<MediaSourceDto>, IpcError> {
    let sources = extractor
        .execute_silent(&url, SILENT_EXTRACTION_TIMEOUT)
        .await?;
    Ok(sources.into_iter().map(MediaSourceDto::from).collect())
}

fn parse_uuid(s: &str) -> Result<Uuid, IpcError> {
    Uuid::parse_str(s)
        .map_err(|_| IpcError::new(IpcErrorKind::Validation, format!("invalid uuid: {s}")))
}
