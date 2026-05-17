use std::path::Path;

use crate::models::BuildJob;

pub async fn clone_repo(job: &BuildJob, work_dir: &Path) -> anyhow::Result<()> {
    let repo_dir = work_dir.join("repo");

    if repo_dir.exists() {
        tokio::fs::remove_dir_all(&repo_dir).await?;
    }

    let git_url = job.git_url.clone();
    let commit_sha = job.commit_sha.clone();

    tokio::task::spawn_blocking(move || {
        let mut fetch_opts = git2::FetchOptions::new();
        let mut proxy_opts = git2::ProxyOptions::new();
        proxy_opts.auto();
        fetch_opts.proxy_options(proxy_opts);

        let mut builder = git2::build::RepoBuilder::new();
        builder.fetch_options(fetch_opts);

        let repo = builder
            .clone(&git_url, &repo_dir)
            .map_err(|e| anyhow::anyhow!("failed to clone repo: {}", e))?;

        let (commit, _) = repo
            .revparse_ext(&commit_sha)
            .map_err(|e| anyhow::anyhow!("failed to resolve commit {}: {}", commit_sha, e))?;

        repo.checkout_tree(
            &commit,
            Some(git2::build::CheckoutBuilder::new().force()),
        )
        .map_err(|e| anyhow::anyhow!("failed to checkout commit: {}", e))?;

        repo.set_head_detached(commit.id())
            .map_err(|e| anyhow::anyhow!("failed to set detached head: {}", e))?;

        Ok::<_, anyhow::Error>(())
    })
    .await??;

    Ok(())
}
