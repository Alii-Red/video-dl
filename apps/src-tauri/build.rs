fn main() {
    // Registers our own #[tauri::command]s for ACL/capability permission
    // generation (`allow-<command>` / `deny-<command>`). Without this, Tauri
    // never creates permission identifiers for app-level commands, so no
    // capability file can reference them by name and every invoke() from a
    // window showing remote content (our page-extraction windows) is denied.
    let attributes = tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "probe_url",
            "extract_from_page",
            "open_interactive_extraction",
            "start_download",
            "resolve_playlist",
            "start_playlist_download",
            "cancel_download",
            "pause_download",
            "resume_download",
            "list_active",
            "list_history",
            "remove_download",
            "retry_download",
            "delete_history_entry",
            "open_containing_folder",
            "open_download_folder",
            "reveal_extension_folder",
            "open_external",
            "rescan_extension",
            "extension_connected",
            "ffmpeg_is_full",
            "get_settings",
            "update_settings",
            "update_ytdlp",
            "report_detected_url",
        ]),
    );
    tauri_build::try_build(attributes).expect("failed to run tauri-build");
}
