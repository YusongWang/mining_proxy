use async_channel::{bounded, Receiver, Sender, TrySendError};
use serde::{Deserialize, Serialize};
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Job {
    phread_id: u32,
    job: String,
}
impl Job {
    pub fn get_job(&mut self) -> String {
        self.job.clone()
    }
    pub fn get_id(&mut self) -> u32 {
        self.phread_id
    }

    pub fn new(id: u32, job: String) -> Job {
        Job { job, phread_id: id }
    }
}
//c: Sender<i32>, r: Receiver<i32>
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

    pub fn send(&self, job: Job) -> Option<()> {
        match self.tx.try_send(job) {
            _ => {}
            Err(e) => match e {
                TrySendError::Full(job) => {
                    if let Ok(_) = self.rx.try_recv() {
                        return self.send(job);
                    } else {
                        return None;
                    }
                }
                TrySendError::Closed(i) => {
                    //println!("线程异常退出了");
                    return None;
                }
            },
        }
        Some(())
    }

    pub async fn recv(&self) -> Result<Job, async_channel::RecvError> {
        self.rx.recv().await
    }
}
