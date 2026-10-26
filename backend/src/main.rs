use ar_navigation::{
    assistant::{Assistant, AssistantConfig, Llm, LlmConfig},
    http::router,
    model::{BuildingId, BuildingSummary},
    navigation::{Navigation, loader::load_building_file},
};
use serde::Deserialize;
use std::{
    env,
    error::Error,
    path::{Path, PathBuf},
    time::Duration,
};
use tracing_subscriber::util::SubscriberInitExt;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BuildingFile {
    id: String,
    name: String,
    path: PathBuf,
}

fn required(name: &str) -> Result<String, Box<dyn Error>> {
    env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("Missing {name}").into())
}

fn number<T: std::str::FromStr>(name: &str, default: T) -> Result<T, Box<dyn Error>> {
    match env::var(name) {
        Ok(value) => value.parse().map_err(|_| format!("Invalid {name}").into()),
        Err(env::VarError::NotPresent) => Ok(default),
        Err(_) => Err(format!("Invalid {name}").into()),
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    // Load before creating worker threads. Existing environment variables win.
    let config_directory = match dotenvy::dotenv() {
        Ok(path) => path.parent().map(Path::to_path_buf),
        Err(error) if error.not_found() => None,
        Err(_) => return Err("Cannot load .env: check file permissions and syntax".into()),
    };
    let directives = match env::var("RUST_LOG") {
        Ok(value) => value,
        Err(env::VarError::NotPresent) => "warn,ar_navigation=info".into(),
        Err(_) => return Err("Invalid RUST_LOG".into()),
    };
    let filter =
        tracing_subscriber::EnvFilter::try_new(directives).map_err(|_| "Invalid RUST_LOG")?;
    ar_navigation::telemetry::subscriber(filter, std::io::stderr)
        .try_init()
        .map_err(|_| "Cannot initialize tracing")?;
    tracing::info!(env_file_loaded = config_directory.is_some(), "配置已加载");
    let result = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(run(config_directory));
    if result.is_err() {
        tracing::error!("服务启动或运行失败");
    }
    result
}

async fn run(config_directory: Option<PathBuf>) -> Result<(), Box<dyn Error>> {
    let mut manifest = PathBuf::from(required("NAVIGATION_BUILDINGS_FILE")?);
    if let Some(directory) = config_directory {
        manifest = directory.join(manifest);
    }
    let files: Vec<BuildingFile> = serde_json::from_str(&std::fs::read_to_string(&manifest)?)?;
    let directory = manifest.parent().unwrap_or(Path::new("."));
    let buildings = files
        .into_iter()
        .map(|file| {
            load_building_file(
                BuildingSummary {
                    id: BuildingId(file.id),
                    name: file.name,
                },
                directory.join(file.path),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let navigation = Navigation::new(buildings)?;
    tracing::info!(
        building_count = navigation.list_buildings().len(),
        "楼栋数据加载完成"
    );
    let llm = Llm::new(LlmConfig {
        base_url: required("LLM_BASE_URL")?,
        api_key: required("LLM_API_KEY")?,
        model: required("LLM_MODEL")?,
        request_options: serde_json::from_str(
            &env::var("LLM_REQUEST_OPTIONS").unwrap_or_else(|_| "{}".into()),
        )
        .map_err(|_| "Invalid LLM_REQUEST_OPTIONS JSON")?,
    })?;
    let defaults = AssistantConfig::default();
    let config = AssistantConfig {
        session_ttl: Duration::from_secs(number(
            "SESSION_TTL_SECONDS",
            defaults.session_ttl.as_secs(),
        )?),
        turn_timeout: Duration::from_secs(number(
            "TURN_TIMEOUT_SECONDS",
            defaults.turn_timeout.as_secs(),
        )?),
        max_sessions: number("MAX_SESSIONS", defaults.max_sessions)?,
        max_concurrent_turns: number("MAX_CONCURRENT_TURNS", defaults.max_concurrent_turns)?,
        max_llm_calls: number("MAX_LLM_CALLS", defaults.max_llm_calls)?,
        max_tool_calls: number("MAX_TOOL_CALLS", defaults.max_tool_calls)?,
        max_history_bytes: number("MAX_HISTORY_BYTES", defaults.max_history_bytes)?,
    };
    let assistant = Assistant::new(navigation, llm, config)?;
    let listener = tokio::net::TcpListener::bind(
        env::var("HTTP_BIND").unwrap_or_else(|_| "127.0.0.1:3000".into()),
    )
    .await?;
    let address = listener.local_addr()?;
    tracing::info!(%address, swagger_ui = "/swagger-ui/", "HTTP服务已启动");
    // Never log conversation IDs, keys, prompts or tool messages.
    let cleanup = tokio::spawn({
        let assistant = assistant.clone();
        async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            loop {
                interval.tick().await;
                assistant.cleanup_expired();
            }
        }
    });
    let result = axum::serve(listener, router(assistant))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await;
    cleanup.abort();
    result?;
    tracing::info!("HTTP服务已停止");
    Ok(())
}
