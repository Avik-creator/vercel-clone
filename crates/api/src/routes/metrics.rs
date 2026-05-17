use crate::{AppState, errors::AppResult};
use axum::{extract::State, response::IntoResponse};

pub async fn metrics(State(state): State<AppState>) -> AppResult<impl IntoResponse> {
    let queued = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM deployments WHERE state = 'queued'")
        .fetch_one(&*state.db)
        .await?;
    let running = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM deployments WHERE state IN ('building', 'uploading')",
    )
    .fetch_one(&*state.db)
    .await?;
    let ready = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM deployments WHERE state = 'ready'")
        .fetch_one(&*state.db)
        .await?;

    Ok(render_metrics(queued, running, ready))
}

fn render_metrics(queued: i64, running: i64, ready: i64) -> String {
    format!(
        "# TYPE build_jobs_queued gauge\nbuild_jobs_queued {}\n# TYPE build_jobs_running gauge\nbuild_jobs_running {}\n# TYPE deployments_ready gauge\ndeployments_ready {}\n",
        queued, running, ready
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_render_prometheus_text_format() {
        let body = render_metrics(2, 3, 5);

        assert!(body.contains("build_jobs_queued 2"));
        assert!(body.contains("build_jobs_running 3"));
        assert!(body.contains("deployments_ready 5"));
    }
}
