#[derive(Debug)]
pub struct Worker {
    pub is_login: bool,
    pub hash: u64,
    pub worker_name: String,
    pub worker_wallet: String,
    pub full_name: String,
    pub share_count: u64,
    pub accept_count: u64,
    pub invalid_count: u64,
}

impl Default for Worker {
    fn default() -> Self {
        Self {
            worker_name: Default::default(),
            worker_wallet: Default::default(),
            full_name: Default::default(),
            share_count: Default::default(),
            accept_count: Default::default(),
            invalid_count: Default::default(),
            is_login: false,
            hash: 0,
        }
    }
}
impl Worker {
    pub fn new() -> Self {
        Worker::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        //assert_eq!(
    }
}
