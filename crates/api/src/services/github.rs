use serde_json::Value;
use secrecy::ExposeSecret;
use sqlx::Row;
use std::collections::HashMap;
use uuid::Uuid;
use crate::{AppState, errors::{AppError, AppResult}, models::BuildJob};

/// Generate an installation access token for cloning private repos
pub async fn get_installation_token(state: &AppState, installation_id: i64) -> AppResult<String> {
    let octocrab = octocrab::OctocrabBuilder::new()
        .app(
            state.config.github_app_id.into(),
            jsonwebtoken::EncodingKey::from_rsa_pem(state.config.github_app_private_key.as_bytes())
                .map_err(|e| AppError::Internal(anyhow::anyhow!("invalid github app private key: {e}")))?,
        )
        .build()
        .map_err(|e| AppError::Internal(anyhow::anyhow!("octocrab build failed: {e}")))?;

    let token = octocrab
        .installation(octocrab::models::InstallationId(installation_id as u64))
        .map_err(|e| AppError::Internal(anyhow::anyhow!("failed to get installation client: {e}")))?
        .installation_token()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("failed to get installation token: {e}")))?;

    Ok(token.expose_secret().to_string())
}

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
    let installation_id = payload["installation"]["id"]
        .as_i64();

    if branch.is_empty() {
        return Err(AppError::BadRequest("missing branch ref in webhook payload".into()));
    }

    tracing::info!(repo = %repo_full_name, sha = %commit_sha, branch = %branch, "push event");

    // Get installation token for cloning private repos
    let github_token = if let Some(inst_id) = installation_id {
        match get_installation_token(state, inst_id).await {
            Ok(token) => Some(token),
            Err(e) => {
                tracing::warn!(error = %e, "failed to get installation token, repo clone may fail for private repos");
                None
            }
        }
    } else {
        None
    };

    // Find projects linked to this repo
    let projects = sqlx::query(
        "SELECT id, build_command, output_dir FROM projects WHERE github_repo = $1",
    )
    .bind(repo_full_name)
    .fetch_all(&*state.db)
    .await?;

    for project in projects {
        let preview_hash: String = (0..8)
            .map(|_| format!("{:x}", rand::random::<u8>() % 16))
            .collect();
        let preview_url = format!("{}-{}.{}", preview_hash, "preview", state.config.base_domain);

        let project_id: Uuid = project.try_get("id")?;
        let build_command: Option<String> = project.try_get("build_command").ok().flatten();
        let output_dir: Option<String> = project.try_get("output_dir").ok().flatten();
        tracing::info!(project_id = %project_id, "triggering deployment");

        let deployment = sqlx::query_as::<_, crate::models::Deployment>(
            r#"
            INSERT INTO deployments
                (project_id, commit_sha, commit_message, branch, state, url, is_production)
            VALUES ($1, $2, $3, $4, 'queued', $5, true)
            RETURNING *
            "#,
        )
        .bind(project_id)
        .bind(commit_sha)
        .bind(commit_message)
        .bind(branch)
        .bind(preview_url)
        .fetch_one(&*state.db)
        .await?;

        // Fetch env vars from projects.env_vars JSONB column
        let env_vars_json: serde_json::Value = sqlx::query_scalar(
            "SELECT env_vars FROM projects WHERE id = $1",
        )
        .bind(project_id)
        .fetch_one(&*state.db)
        .await?;

        let env_vars: HashMap<String, String> = env_vars_json
            .as_object()
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| {
                        v.as_str().map(|s| (k.clone(), s.to_string()))
                    })
                    .collect()
            })
            .unwrap_or_default();

        // Dispatch build job via NATS JetStream
        let git_url = format!("https://github.com/{}.git", repo_full_name);

        let build_job = BuildJob {
            deployment_id: deployment.id,
            project_id,
            git_url,
            commit_sha: commit_sha.to_string(),
            branch: branch.to_string(),
            build_command,
            output_dir,
            github_token: github_token.clone(),
            env_vars,
        };

        state.nats.publish_job(&build_job).await?;

        tracing::info!(
            deployment_id = %deployment.id,
            commit = %commit_sha,
            repo = %repo_full_name,
            env_var_count = build_job.env_vars.len(),
            "build job published to NATS from webhook"
        );
    }

    Ok(())
}

pub async fn handle_pull_request(_state: &AppState, payload: Value) -> AppResult<()> {
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
