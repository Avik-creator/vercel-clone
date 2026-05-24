use std::path::{Path, PathBuf};

use async_nats::ConnectOptions;

pub fn apply_tls(opts: ConnectOptions, ca_file: Option<&str>) -> anyhow::Result<ConnectOptions> {
    let Some(ca_path) = ca_file.filter(|p| !p.is_empty() && Path::new(p).exists()) else {
        return Ok(opts);
    };
    Ok(opts
        .require_tls(true)
        .add_root_certificates(PathBuf::from(ca_path)))
}
