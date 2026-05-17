use crate::{
    AppState,
    errors::{AppError, AppResult, NotFoundExt},
    models::{
        BuildCallbackRequest, BuildJob, CreateDeploymentRequest, Deployment, DeploymentState,
        EnvVarTarget,
    },
    services::github as github_service,
    services::projects as project_service,
};
use rand::Rng;
use sqlx::Row;
use std::collections::HashMap;
use uuid::Uuid;

pub async fn list_for_user(state: &AppState, user_id: Uuid) -> AppResult<Vec<Deployment>> {
    let rows = sqlx::query_as::<_, Deployment>(
        r#"
        SELECT d.* FROM deployments d
        JOIN projects p ON d.project_id = p.id
        WHERE p.owner_id = $1
        ORDER BY d.created_at DESC
        LIMIT 50
        "#,
    )
    .bind(user_id)
    .fetch_all(&*state.db)
    .await?;
    Ok(rows)
}

pub async fn list_for_project(
    state: &AppState,
    user_id: Uuid,
    project_id: Uuid,
) -> AppResult<Vec<Deployment>> {
    // Verify ownership
    sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM projects WHERE id = $1 AND owner_id = $2)",
    )
    .bind(project_id)
    .bind(user_id)
    .fetch_one(&*state.db)
    .await?
    .then_some(())
    .ok_or_else(|| AppError::NotFound("project not found".into()))?;

    let rows = sqlx::query_as::<_, Deployment>(
        "SELECT * FROM deployments WHERE project_id = $1 ORDER BY created_at DESC LIMIT 50",
    )
    .bind(project_id)
    .fetch_all(&*state.db)
    .await?;
    Ok(rows)
}

pub async fn create(
    state: &AppState,
    user_id: Uuid,
    req: CreateDeploymentRequest,
) -> AppResult<Deployment> {
    let project_id = req
        .project_id
        .ok_or_else(|| AppError::BadRequest("missing project_id".into()))?;

    // Verify project ownership
    let project = sqlx::query(
        "SELECT id, build_command, output_dir, github_repo, github_installation_id
         FROM projects WHERE id = $1 AND owner_id = $2",
    )
    .bind(project_id)
    .bind(user_id)
    .fetch_one(&*state.db)
    .await
    .or_not_found("project")?;

    let _project_id: Uuid = project.try_get("id")?;
    let github_repo: Option<String> = project.try_get("github_repo")?;
    let github_installation_id: Option<i64> =
        project.try_get("github_installation_id").ok().flatten();

    // Get installation token for cloning private repos
    let github_token = if let Some(inst_id) = github_installation_id {
        match github_service::get_installation_token(state, inst_id).await {
            Ok(token) => Some(token),
            Err(e) => {
                tracing::warn!(error = %e, "failed to get installation token, repo clone may fail for private repos");
                None
            }
        }
    } else {
        None
    };

    // Generate random short hash for preview URL (like Vercel does)
    let preview_hash: String = (0..8)
        .map(|_| format!("{:x}", rand::thread_rng().gen_range(0..16)))
        .collect();
    let preview_url = format!(
        "{}-{}.{}",
        preview_hash, "preview", state.config.base_domain
    );

    let deployment = sqlx::query_as::<_, Deployment>(
        r#"
        INSERT INTO deployments
            (project_id, commit_sha, commit_message, branch,
             state, url, is_production)
        VALUES ($1, $2, $3, $4, 'queued', $5, false)
        RETURNING *
        "#,
    )
    .bind(project_id)
    .bind(&req.commit_sha)
    .bind(&req.commit_message)
    .bind(&req.branch)
    .bind(&preview_url)
    .fetch_one(&*state.db)
    .await?;

    // Fetch env vars for this project (build + all targets)
    let env_var_entries = project_service::get_env_vars(state, user_id, project_id).await?;
    let env_vars: HashMap<String, String> = env_var_entries
        .into_iter()
        .filter(|e| matches!(e.target, EnvVarTarget::Build | EnvVarTarget::All))
        .map(|e| (e.key, e.value))
        .collect();

    // Dispatch build job via NATS JetStream
    let git_url = if let Some(ref repo) = github_repo {
        format!("https://github.com/{}.git", repo)
    } else {
        String::new()
    };

    let build_job = BuildJob {
        deployment_id: deployment.id,
        project_id,
        git_url,
        commit_sha: req.commit_sha.clone(),
        branch: req.branch.clone(),
        build_command: project.try_get("build_command").ok().flatten(),
        output_dir: project.try_get("output_dir").ok().flatten(),
        github_token,
        env_vars,
    };

    state.nats.publish_job(&build_job).await?;

    tracing::info!(
        deployment_id = %deployment.id,
        commit = %req.commit_sha,
        repo = ?github_repo,
        env_var_count = build_job.env_vars.len(),
        "build job published to NATS"
    );

    Ok(deployment)
}

pub async fn get_for_user(state: &AppState, user_id: Uuid, id: Uuid) -> AppResult<Deployment> {
    sqlx::query_as::<_, Deployment>(
        r#"
        SELECT d.* FROM deployments d
        JOIN projects p ON d.project_id = p.id
        WHERE d.id = $1 AND p.owner_id = $2
        "#,
    )
    .bind(id)
    .bind(user_id)
    .fetch_one(&*state.db)
    .await
    .or_not_found("deployment")
}

pub async fn cancel(state: &AppState, user_id: Uuid, id: Uuid) -> AppResult<()> {
    let rows = sqlx::query(
        r#"
        UPDATE deployments SET state = 'cancelled', updated_at = NOW()
        WHERE id = $1
          AND state IN ('queued', 'building')
          AND project_id IN (SELECT id FROM projects WHERE owner_id = $2)
        "#,
    )
    .bind(id)
    .bind(user_id)
    .execute(&*state.db)
    .await?
    .rows_affected();

    if rows == 0 {
        return Err(AppError::NotFound(
            "deployment not found or not cancellable".into(),
        ));
    }
    Ok(())
}

pub async fn promote_to_production(state: &AppState, user_id: Uuid, id: Uuid) -> AppResult<()> {
    let deploy = get_for_user(state, user_id, id).await?;

    if deploy.state != DeploymentState::Ready {
        return Err(AppError::BadRequest(
            "only ready deployments can be promoted".into(),
        ));
    }

    // Demote current production deployment for this project
    sqlx::query(
        "UPDATE deployments SET is_production = false, updated_at = NOW()
         WHERE project_id = $1 AND is_production = true",
    )
    .bind(deploy.project_id)
    .execute(&*state.db)
    .await?;

    // Promote this one
    sqlx::query("UPDATE deployments SET is_production = true, updated_at = NOW() WHERE id = $1")
        .bind(id)
        .execute(&*state.db)
        .await?;

    Ok(())
}

/// Called by build workers when build state changes
pub async fn handle_build_callback(state: &AppState, req: BuildCallbackRequest) -> AppResult<()> {
    let log_update =
        if let Some(chunk) = &req.log_chunk {
            Some(sqlx::query(
            "UPDATE deployments SET build_log = COALESCE(build_log, '') || $1 WHERE id = $2",
        )
        .bind(chunk)
        .bind(req.deployment_id)
        .execute(&*state.db)
        .await?)
        } else {
            None
        };

    let _ = log_update;

    let current_state =
        sqlx::query_scalar::<_, DeploymentState>("SELECT state FROM deployments WHERE id = $1")
            .bind(req.deployment_id)
            .fetch_one(&*state.db)
            .await
            .or_not_found("deployment")?;

    if !is_valid_transition(current_state, req.state.clone()) {
        return Err(AppError::BadRequest(
            "invalid deployment state transition".into(),
        ));
    }

    let state_val = req.state.clone();
    sqlx::query(
        r#"
        UPDATE deployments SET
            state = $2,
            artifact_key = COALESCE($3, artifact_key),
            image_ref = COALESCE($4, image_ref),
            build_started_at  = CASE WHEN $2::text = 'building' THEN NOW() ELSE build_started_at END,
            build_finished_at = CASE WHEN $2::text IN ('ready', 'error') THEN NOW() ELSE build_finished_at END,
            updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(req.deployment_id)
    .bind(req.state)
    .bind(artifact_key_for_update(&req))
    .bind(image_ref_for_update(&req))
    .execute(&*state.db)
    .await?;

    tracing::info!(
        deployment = %req.deployment_id,
        state = ?state_val,
        "build callback processed"
    );

    Ok(())
}

fn artifact_key_for_update(req: &BuildCallbackRequest) -> Option<&str> {
    req.artifact_key.as_deref()
}

fn image_ref_for_update(req: &BuildCallbackRequest) -> Option<&str> {
    req.image_ref.as_deref()
}

fn is_valid_transition(from: DeploymentState, to: DeploymentState) -> bool {
    use DeploymentState::*;

    match from {
        Queued => matches!(to, Queued | Building | Error | Cancelled),
        Building => matches!(to, Building | Uploading | Ready | Error | Cancelled),
        Uploading => matches!(to, Uploading | Ready | Error | Cancelled),
        Ready | Error | Cancelled => from == to,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ready_callback_persists_artifact_key() {
        let req = BuildCallbackRequest {
            deployment_id: Uuid::nil(),
            state: DeploymentState::Ready,
            log_chunk: None,
            artifact_key: Some("deployments/abc".to_string()),
            image_ref: Some("localhost:5000/deployment-abc:latest".to_string()),
        };

        assert_eq!(artifact_key_for_update(&req), Some("deployments/abc"));
        assert_eq!(
            image_ref_for_update(&req),
            Some("localhost:5000/deployment-abc:latest")
        );
    }

    #[test]
    fn terminal_deployments_cannot_reenter_building() {
        assert!(!is_valid_transition(
            DeploymentState::Cancelled,
            DeploymentState::Building
        ));
        assert!(!is_valid_transition(
            DeploymentState::Ready,
            DeploymentState::Building
        ));
        assert!(is_valid_transition(
            DeploymentState::Queued,
            DeploymentState::Building
        ));
    }

}
