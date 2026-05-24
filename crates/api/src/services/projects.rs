use crate::{
    AppState,
    errors::{AppResult, NotFoundExt},
    models::{
        CreateProjectRequest, EnvVarEntry, EnvVarTarget, LinkGithubRequest, Project,
        UpdateProjectRequest,
    },
};
use uuid::Uuid;

pub async fn list_for_user(state: &AppState, user_id: Uuid) -> AppResult<Vec<Project>> {
    let projects = sqlx::query_as::<_, Project>(
        "SELECT * FROM projects WHERE owner_id = $1 ORDER BY created_at DESC",
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
             build_command, output_dir, production_branch, env_vars)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, '[]')
        RETURNING *
        "#,
    )
    .bind(owner_id)
    .bind(&req.name)
    .bind(&slug)
    .bind(&req.github_repo)
    .bind(&req.framework)
    .bind(req.build_command.as_deref().unwrap_or("npm run build"))
    .bind(req.output_dir.as_deref().unwrap_or("dist"))
    .bind(req.production_branch.as_deref().unwrap_or("main"))
    .fetch_one(&*state.db)
    .await?;

    Ok(project)
}

pub async fn get_for_user(state: &AppState, user_id: Uuid, project_id: Uuid) -> AppResult<Project> {
    sqlx::query_as::<_, Project>("SELECT * FROM projects WHERE id = $1 AND owner_id = $2")
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
            name              = COALESCE($3, name),
            build_command     = COALESCE($4, build_command),
            output_dir        = COALESCE($5, output_dir),
            root_dir          = COALESCE($6, root_dir),
            production_branch = COALESCE($7, production_branch),
            updated_at        = NOW()
        WHERE id = $1 AND owner_id = $2
        RETURNING *
        "#,
    )
    .bind(project_id)
    .bind(user_id)
    .bind(req.name)
    .bind(req.build_command)
    .bind(req.output_dir)
    .bind(req.root_dir)
    .bind(req.production_branch)
    .fetch_one(&*state.db)
    .await?;

    Ok(project)
}

pub async fn delete(state: &AppState, user_id: Uuid, project_id: Uuid) -> AppResult<()> {
    let rows = sqlx::query("DELETE FROM projects WHERE id = $1 AND owner_id = $2")
        .bind(project_id)
        .bind(user_id)
        .execute(&*state.db)
        .await?
        .rows_affected();

    if rows == 0 {
        return Err(crate::errors::AppError::NotFound(
            "project not found".into(),
        ));
    }
    Ok(())
}

pub async fn get_env_vars(
    state: &AppState,
    user_id: Uuid,
    project_id: Uuid,
) -> AppResult<Vec<EnvVarEntry>> {
    let project = get_for_user(state, user_id, project_id).await?;

    let env_vars: Vec<EnvVarEntry> = serde_json::from_value(project.env_vars).unwrap_or_default();

    Ok(env_vars)
}

pub async fn update_env_vars(
    state: &AppState,
    user_id: Uuid,
    project_id: Uuid,
    env_vars: Vec<EnvVarEntry>,
) -> AppResult<Vec<EnvVarEntry>> {
    let _project = get_for_user(state, user_id, project_id).await?;

    let json =
        serde_json::to_value(&env_vars).map_err(|e| crate::errors::AppError::Internal(e.into()))?;

    sqlx::query(
        "UPDATE projects SET env_vars = $1, updated_at = NOW() WHERE id = $2 AND owner_id = $3",
    )
    .bind(json)
    .bind(project_id)
    .bind(user_id)
    .execute(&*state.db)
    .await?;

    Ok(env_vars)
}

pub async fn add_env_var(
    state: &AppState,
    user_id: Uuid,
    project_id: Uuid,
    new_var: EnvVarEntry,
) -> AppResult<Vec<EnvVarEntry>> {
    let project = get_for_user(state, user_id, project_id).await?;

    let mut env_vars: Vec<EnvVarEntry> =
        serde_json::from_value(project.env_vars).unwrap_or_default();

    // Update existing key or append new one
    if let Some(existing) = env_vars.iter_mut().find(|v| v.key == new_var.key) {
        existing.value = new_var.value;
        existing.target = new_var.target;
    } else {
        env_vars.push(new_var);
    }

    let json =
        serde_json::to_value(&env_vars).map_err(|e| crate::errors::AppError::Internal(e.into()))?;

    sqlx::query(
        "UPDATE projects SET env_vars = $1, updated_at = NOW() WHERE id = $2 AND owner_id = $3",
    )
    .bind(json)
    .bind(project_id)
    .bind(user_id)
    .execute(&*state.db)
    .await?;

    Ok(env_vars)
}

pub async fn delete_env_var(
    state: &AppState,
    user_id: Uuid,
    project_id: Uuid,
    key: &str,
) -> AppResult<Vec<EnvVarEntry>> {
    let project = get_for_user(state, user_id, project_id).await?;

    let mut env_vars: Vec<EnvVarEntry> =
        serde_json::from_value(project.env_vars).unwrap_or_default();

    let initial_len = env_vars.len();
    env_vars.retain(|v| v.key != key);

    if env_vars.len() == initial_len {
        return Err(crate::errors::AppError::NotFound(format!(
            "env var '{}' not found",
            key
        )));
    }

    let json =
        serde_json::to_value(&env_vars).map_err(|e| crate::errors::AppError::Internal(e.into()))?;

    sqlx::query(
        "UPDATE projects SET env_vars = $1, updated_at = NOW() WHERE id = $2 AND owner_id = $3",
    )
    .bind(json)
    .bind(project_id)
    .bind(user_id)
    .execute(&*state.db)
    .await?;

    Ok(env_vars)
}

pub async fn link_github(
    state: &AppState,
    user_id: Uuid,
    project_id: Uuid,
    req: LinkGithubRequest,
) -> AppResult<Project> {
    let _project = get_for_user(state, user_id, project_id).await?;

    let project = sqlx::query_as::<_, Project>(
        r#"
        UPDATE projects SET
            github_repo = $3,
            github_installation_id = $4,
            updated_at = NOW()
        WHERE id = $1 AND owner_id = $2
        RETURNING *
        "#,
    )
    .bind(project_id)
    .bind(user_id)
    .bind(&req.github_repo)
    .bind(req.installation_id)
    .fetch_one(&*state.db)
    .await?;

    Ok(project)
}

pub fn env_map_for_targets(
    env_vars: &[EnvVarEntry],
    targets: &[EnvVarTarget],
) -> std::collections::HashMap<String, String> {
    env_vars
        .iter()
        .filter(|entry| targets.contains(&entry.target))
        .map(|entry| (entry.key.clone(), entry.value.clone()))
        .collect()
}

pub fn parse_dotenv(content: &str) -> AppResult<Vec<EnvVarEntry>> {
    let mut entries = Vec::new();

    for (line_no, raw_line) in content.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let line = line.strip_prefix("export ").unwrap_or(line).trim();
        let Some((key, value)) = line.split_once('=') else {
            return Err(crate::errors::AppError::BadRequest(format!(
                "invalid .env syntax on line {}: expected KEY=VALUE",
                line_no + 1
            )));
        };

        let key = key.trim();
        if key.is_empty() {
            return Err(crate::errors::AppError::BadRequest(format!(
                "invalid .env syntax on line {}: empty key",
                line_no + 1
            )));
        }

        let value = parse_dotenv_value(value.trim());
        entries.push(EnvVarEntry {
            key: key.to_string(),
            value,
            target: EnvVarTarget::All,
        });
    }

    Ok(entries)
}

fn parse_dotenv_value(raw: &str) -> String {
    if raw.len() >= 2 {
        let bytes = raw.as_bytes();
        let quote = bytes[0];
        if (quote == b'"' || quote == b'\'') && bytes[bytes.len() - 1] == quote {
            return raw[1..raw.len() - 1].to_string();
        }
    }
    raw.to_string()
}

pub async fn import_env_from_dotenv(
    state: &AppState,
    user_id: Uuid,
    project_id: Uuid,
    content: &str,
    target: EnvVarTarget,
    merge: bool,
) -> AppResult<Vec<EnvVarEntry>> {
    let mut parsed = parse_dotenv(content)?;
    for entry in &mut parsed {
        entry.target = target.clone();
    }

    if merge {
        let mut env_vars = get_env_vars(state, user_id, project_id).await?;
        for entry in parsed {
            if let Some(existing) = env_vars.iter_mut().find(|v| v.key == entry.key) {
                *existing = entry;
            } else {
                env_vars.push(entry);
            }
        }
        update_env_vars(state, user_id, project_id, env_vars).await
    } else {
        update_env_vars(state, user_id, project_id, parsed).await
    }
}

fn slugify(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_dotenv_reads_basic_pairs() {
        let content = "FOO=bar\n# comment\nexport BAZ=\"hello world\"\n";
        let parsed = parse_dotenv(content).unwrap();

        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].key, "FOO");
        assert_eq!(parsed[0].value, "bar");
        assert_eq!(parsed[1].key, "BAZ");
        assert_eq!(parsed[1].value, "hello world");
    }

    #[test]
    fn parse_dotenv_rejects_invalid_line() {
        let err = parse_dotenv("NOT_VALID").unwrap_err();
        assert!(err.to_string().contains("KEY=VALUE"));
    }
}
