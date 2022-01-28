pub mod handles;

#[derive(Clone)]
pub struct AppState {
    pub global_count:
        std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, tokio::process::Child>>>,
}
