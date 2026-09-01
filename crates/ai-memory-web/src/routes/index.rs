//! `GET /` — project list cards.

use std::sync::Arc;

use askama::Template;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Html;

use crate::state::WebState;
use crate::templates::{BackupNotice, ProjectCard, ProjectsView, humanize, project_href};

/// Handler for `GET /`.
pub(crate) async fn handler(
    State(state): State<Arc<WebState>>,
) -> Result<Html<String>, StatusCode> {
    let summaries = state
        .reader
        .list_projects_with_stats()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let projects = summaries
        .into_iter()
        .map(|s| {
            let last_updated_relative = s.last_updated.as_deref().map(humanize).unwrap_or_default();
            let href = project_href(&s.workspace_name, &s.project_name);
            ProjectCard {
                workspace: s.workspace_name,
                project: s.project_name,
                page_count: s.page_count,
                last_updated_relative,
                href,
            }
        })
        .collect();

    let backup_notice = backup_notice(&state);
    let html = ProjectsView {
        projects,
        backup_notice,
    }
    .render()
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Html(html))
}

/// The migration's backup receipt, surfaced while the archive is still
/// on disk; the notice disappears on its own once the user deletes the
/// file (docs/okf.md).
fn backup_notice(state: &WebState) -> Option<BackupNotice> {
    let receipt = ai_memory_wiki::backup::BackupReceipt::load(state.wiki.data_dir())?;
    if !receipt.archive_present() {
        return None;
    }
    Some(BackupNotice {
        archive_path: receipt.archive_path.display().to_string(),
        size_human: human_bytes(receipt.size_bytes),
        created_at: receipt.created_at,
    })
}

fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}
