//! gateway-serve bin — WS-транспорт кокпита (M-28, D1/D6). Тонкая обёртка над `gateway_serve::server`.
//!
//! Каркас (architect): bin существует и компилируется (acceptance task #4). Тело — engine-dev (task #4):
//! собрать `ServeConfig` из env/args (`addr`/`journal_dir`/`filter`/`selector`/`decoding_key`), поднять
//! tokio-runtime → `server::bind(cfg).await` → `server.serve().await`. Read-only, stateless по юзеру.

fn main() -> std::io::Result<()> {
    unimplemented!(
        "M-28 task #4 (engine-dev): parse ServeConfig из env → tokio bind+serve (gateway_serve::server)"
    )
}
