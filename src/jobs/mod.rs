use async_channel::{bounded, Receiver, Sender, TrySendError};

use serde::{Deserialize, Serialize};
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Job {
    phread_id: u32,
    job: String,
    diff: u64,
}

impl Job {
    pub fn get_job(&self) -> String {
        self.job.clone()
    }

    pub fn get_id(&self) -> u32 {
        self.phread_id
    }

    pub fn get_diff(&self) -> u64 {
        self.diff
    }

    pub fn new(id: u32, job: String, diff: u64) -> Job {
        Job {
            job,
            phread_id: id,
            diff,
        }
    }
}

pub struct JobQueue {
    tx: Sender<Job>,
    rx: Receiver<Job>,
}

impl JobQueue {
    pub fn new(cap: usize) -> Self {
        let (tx, rx) = bounded::<Job>(cap);
        Self { tx, rx }
    }

    pub fn try_recv(&self) -> Option<Job> {
        if let Ok(job) = self.rx.try_recv() {
            return Some(job);
        }
        None
    }

    pub fn try_send(&self, job: Job) -> Option<()> {
        match self.tx.try_send(job) {
            _ => {}
            Err(e) => match e {
                TrySendError::Full(_job) => {
                    // if let Ok(_) = self.rx.try_recv() {
                    //     return self.send(job);
                    // } else {
                    //     return None;
                    // }
                    return Some(());
                }
                TrySendError::Closed(_i) => {
                    //println!("线程异常退出了");
                    return None;
                }
            },
        }
        Some(())
    }

    // pub async fn send(&self, job: Job) -> Option<()> {
    //     self.tx.send(job).await
    // }

    pub async fn recv(&self) -> Result<Job, async_channel::RecvError> {
        self.rx.recv().await
    }
}
