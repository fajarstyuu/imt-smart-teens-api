//! Runtime entry point only; application assembly and lifecycle belong to bootstrap.
use imt_smart_teens_backend::bootstrap;
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    bootstrap::run().await
}
