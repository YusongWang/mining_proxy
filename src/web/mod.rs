use crate::{state::Worker, util::config::Settings};

pub mod data;
pub mod handles;
// pub struct AppState {
//     pub global_count: std::sync::Arc<
//         std::sync::Mutex<std::collections::HashMap<String, OnlineWorker>>,
//     >,
// }

pub type AppState = std::sync::Arc<
    std::sync::Mutex<std::collections::HashMap<String, OnlineWorker>>,
>;

pub struct OnlineWorker {
    pub child: tokio::process::Child,
    pub workers: Vec<Worker>,
    pub online: u32,
    pub config: Settings,
}
