use serde_json::Value;
use crate::{AppState, errors::{AppError, AppResult}};

/// push event → trigger a deployment for the linked project
pub async fn handle_push(state: &AppState, payload: Value) -> AppResult<()> {
    let repo_full_name = payload["repository"]["full_name"]
        .as_str()
        .ok_or_else(|| AppError::BadRequest("missing repository.full_name in webhook payload".into()))?;
    let commit_sha = payload["after"]
        .as_str()
        .ok_or_else(|| AppError::BadRequest("missing 'after' (commit sha) in webhook payload".into()))?;
    let commit_message = payload["head_commit"]["message"]
        .as_str()
        .unwrap_or("unknown commit");
    let branch = payload["ref"]
        .as_str()
        .unwrap_or("")
        .trim_start_matches("refs/heads/");

    if branch.is_empty() {
        return Err(AppError::BadRequest("missing branch ref in webhook payload".into()));
    }

    tracing::info!(repo = %repo_full_name, sha = %commit_sha, branch = %branch, "push event");

    // Find projects linked to this repo
    let projects = sqlx::query!(
        "SELECT id FROM projects WHERE github_repo = $1",
        repo_full_name
    )
    .fetch_all(&*state.db)
    .await?;

    for project in projects {
        let preview_hash: String = (0..8)
            .map(|_| format!("{:x}", rand::random::<u8>() % 16))
            .collect();
        let preview_url = format!("{}-{}.{}", preview_hash, "preview", state.config.base_domain);

        tracing::info!(project_id = %project.id, "triggering deployment");

        sqlx::query!(
            r#"
            INSERT INTO deployments
                (project_id, commit_sha, commit_message, branch, state, url, is_production)
            VALUES ($1, $2, $3, $4, 'queued', $5, false)
            "#,
            project.id,
            commit_sha,
            commit_message,
            branch,
            preview_url,
        )
        .execute(&*state.db)
        .await?;

        // TODO: dispatch to build queue (NATS / internal channel)
    }

    Ok(())
}

pub async fn handle_pull_request(state: &AppState, payload: Value) -> AppResult<()> {
    let action = payload["action"].as_str().unwrap_or_default();
    let pr_number = payload["pull_request"]["number"].as_u64().unwrap_or_default();
    let repo = payload["repository"]["full_name"].as_str().unwrap_or_default();

    tracing::info!(action = %action, pr = %pr_number, repo = %repo, "pull_request event");

    match action {
        "opened" | "synchronize" | "reopened" => {
            // TODO: create preview deployment for this PR
        }
        "closed" => {
            // TODO: tear down PR preview deployment
        }
        _ => {}
    }

    Ok(())
}

pub async fn handle_installation(_state: &AppState, payload: Value) -> AppResult<()> {
    let action = payload["action"].as_str().unwrap_or_default();
    let installation_id = payload["installation"]["id"].as_u64().unwrap_or_default();

    tracing::info!(action = %action, installation = %installation_id, "installation event");

    // TODO: store / remove installation record for this user
    Ok(())
}

pub async fn handle_installation_repositories(_state: &AppState, payload: Value) -> AppResult<()> {
    let action = payload["action"].as_str().unwrap_or_default();
    tracing::info!(action = %action, "installation_repositories event");
    // TODO: sync available repos list for the installation
    Ok(())
}
