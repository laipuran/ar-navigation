//! 日志输出与隐私过滤；应用自行记录脱敏的LLM诊断信息。

use tracing::Subscriber;
use tracing_subscriber::{EnvFilter, Layer, fmt::MakeWriter, layer::SubscriberExt};

pub fn subscriber<W>(filter: EnvFilter, writer: W) -> impl Subscriber + Send + Sync
where
    W: for<'a> MakeWriter<'a> + Send + Sync + 'static,
{
    tracing_subscriber::registry().with(filter).with(
        tracing_subscriber::fmt::layer()
            .with_ansi(false)
            .with_writer(writer)
            // SDK即使在warn级别也可能输出响应正文，不能只依赖RUST_LOG降级。
            .with_filter(tracing_subscriber::filter::filter_fn(|metadata| {
                !metadata.target().starts_with("async_openai")
            })),
    )
}
