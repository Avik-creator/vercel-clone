use uuid::Uuid;
use crate::{
    AppState,
    errors::{AppResult, NotFoundExt},
    models::project::{CreateProjectRequest, Project, UpdateProjectRequest},
};

pub async fn list_for_user(state: &AppState, user_id: Uuid) -> AppResult<Vec<Project>> {
    let projects = sqlx::query_as::<_, Project>(
        "SELECT * FROM projects WHERE owner_id = $1 ORDER BY created_at DESC"
    )
    .bind(user_id)
    .fetch_all(&*state.db)
    .await?;
    Ok(projects)
}

pub async fn create(
    state: &AppState,
    owner_id: Uuid,
    req: CreateProjectRequest,
) -> AppResult<Project> {
    let slug = slugify(&req.name);

    let project = sqlx::query_as::<_, Project>(
        r#"
        INSERT INTO projects
            (owner_id, name, slug, github_repo, framework,
             build_command, output_dir, env_vars)
        VALUES ($1, $2, $3, $4, $5, $6, $7, '{}')
        RETURNING *
        "#
    )
    .bind(owner_id)
    .bind(&req.name)
    .bind(&slug)
    .bind(&req.github_repo)
    .bind(&req.framework)
    .bind(req.build_command.as_deref().unwrap_or("npm run build"))
    .bind(req.output_dir.as_deref().unwrap_or("dist"))
    .fetch_one(&*state.db)
    .await?;

    Ok(project)
}

pub async fn get_for_user(
    state: &AppState,
    user_id: Uuid,
    project_id: Uuid,
) -> AppResult<Project> {
    sqlx::query_as::<_, Project>(
        "SELECT * FROM projects WHERE id = $1 AND owner_id = $2"
    )
    .bind(project_id)
    .bind(user_id)
    .fetch_one(&*state.db)
    .await
    .or_not_found("project")
}

pub async fn update(
    state: &AppState,
    user_id: Uuid,
    project_id: Uuid,
    req: UpdateProjectRequest,
) -> AppResult<Project> {
    // Verify ownership first
    get_for_user(state, user_id, project_id).await?;

    let project = sqlx::query_as::<_, Project>(
        r#"
        UPDATE projects SET
            name         = COALESCE($3, name),
            build_command = COALESCE($4, build_command),
            output_dir   = COALESCE($5, output_dir),
            root_dir     = COALESCE($6, root_dir),
            updated_at   = NOW()
        WHERE id = $1 AND owner_id = $2
        RETURNING *
        "#
    )
    .bind(project_id)
    .bind(user_id)
    .bind(req.name)
    .bind(req.build_command)
    .bind(req.output_dir)
    .bind(req.root_dir)
    .fetch_one(&*state.db)
    .await?;

    Ok(project)
}

pub async fn delete(state: &AppState, user_id: Uuid, project_id: Uuid) -> AppResult<()> {
    let rows = sqlx::query!(
        "DELETE FROM projects WHERE id = $1 AND owner_id = $2",
        project_id, user_id
    )
    .execute(&*state.db)
    .await?
    .rows_affected();

    if rows == 0 {
        return Err(crate::errors::AppError::NotFound("project not found".into()));
    }
    Ok(())
}

fn slugify(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}
