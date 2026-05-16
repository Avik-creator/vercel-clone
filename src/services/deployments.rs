use rand::Rng;
use sqlx::Row;
use uuid::Uuid;
use crate::{
    AppState,
    errors::{AppError, AppResult, NotFoundExt},
    models::{BuildCallbackRequest, CreateDeploymentRequest, Deployment, DeploymentState},
};

pub async fn list_for_user(state: &AppState, user_id: Uuid) -> AppResult<Vec<Deployment>> {
    let rows = sqlx::query_as::<_, Deployment>(
        r#"
        SELECT d.* FROM deployments d
        JOIN projects p ON d.project_id = p.id
        WHERE p.owner_id = $1
        ORDER BY d.created_at DESC
        LIMIT 50
        "#
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
        "SELECT EXISTS(SELECT 1 FROM projects WHERE id = $1 AND owner_id = $2)"
    )
    .bind(project_id)
    .bind(user_id)
    .fetch_one(&*state.db)
    .await?
    .then_some(())
    .ok_or_else(|| AppError::NotFound("project not found".into()))?;

    let rows = sqlx::query_as::<_, Deployment>(
        "SELECT * FROM deployments WHERE project_id = $1 ORDER BY created_at DESC LIMIT 50"
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
    let project_id = req.project_id.ok_or_else(|| AppError::BadRequest("missing project_id".into()))?;

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

    // Generate random short hash for preview URL (like Vercel does)
    let preview_hash: String = (0..8)
        .map(|_| format!("{:x}", rand::thread_rng().gen_range(0..16)))
        .collect();
    let preview_url = format!("{}-{}.{}", preview_hash, "preview", state.config.base_domain);

    let deployment = sqlx::query_as::<_, Deployment>(
        r#"
        INSERT INTO deployments
            (project_id, commit_sha, commit_message, branch,
             state, url, is_production)
        VALUES ($1, $2, $3, $4, 'queued', $5, false)
        RETURNING *
        "#
    )
    .bind(project_id)
    .bind(&req.commit_sha)
    .bind(&req.commit_message)
    .bind(&req.branch)
    .bind(&preview_url)
    .fetch_one(&*state.db)
    .await?;

    // Dispatch build job
    // TODO: push to NATS JetStream or internal queue
    tracing::info!(
        deployment_id = %deployment.id,
        commit = %req.commit_sha,
        repo = ?github_repo,
        "dispatching build job"
    );

    Ok(deployment)
}

pub async fn get_for_user(state: &AppState, user_id: Uuid, id: Uuid) -> AppResult<Deployment> {
    sqlx::query_as::<_, Deployment>(
        r#"
        SELECT d.* FROM deployments d
        JOIN projects p ON d.project_id = p.id
        WHERE d.id = $1 AND p.owner_id = $2
        "#
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
        return Err(AppError::NotFound("deployment not found or not cancellable".into()));
    }
    Ok(())
}

pub async fn promote_to_production(state: &AppState, user_id: Uuid, id: Uuid) -> AppResult<()> {
    let deploy = get_for_user(state, user_id, id).await?;

    if deploy.state != DeploymentState::Ready {
        return Err(AppError::BadRequest("only ready deployments can be promoted".into()));
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
    sqlx::query(
        "UPDATE deployments SET is_production = true, updated_at = NOW() WHERE id = $1",
    )
    .bind(id)
    .execute(&*state.db)
    .await?;

    Ok(())
}

/// Called by build workers when build state changes
pub async fn handle_build_callback(state: &AppState, req: BuildCallbackRequest) -> AppResult<()> {
    let log_update = if let Some(chunk) = &req.log_chunk {
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

    let state_val = req.state.clone();
    sqlx::query(
        r#"
        UPDATE deployments SET
            state = $2,
            build_started_at  = CASE WHEN $2::text = 'building' THEN NOW() ELSE build_started_at END,
            build_finished_at = CASE WHEN $2::text IN ('ready', 'error') THEN NOW() ELSE build_finished_at END,
            updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(req.deployment_id)
    .bind(req.state)
    .execute(&*state.db)
    .await?;

    tracing::info!(
        deployment = %req.deployment_id,
        state = ?state_val,
        "build callback processed"
    );

    Ok(())
}
