mod version {
    include!(concat!(env!("OUT_DIR"), "/version.rs"));
}
use std::vec;

use anyhow::{bail, Result};
use clap::{crate_description, crate_name, crate_version, App, Arg, ArgMatches};
use futures::future;
use serde::{Deserialize, Serialize};
use tokio::io::{split, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadHalf, WriteHalf};

use tokio::net::TcpSocket;
use tokio::time::sleep;

async fn client_to_server<W>(mut w: WriteHalf<W>, i: i32) -> Result<()>
where
    W: AsyncWrite,
{
    let mut is_login = false;
    loop {
        if !is_login {
            let login = Client {
                id: 1,
                method: "eth_submitLogin".into(),
                params: vec![
                    "0xb0B91c95D2D0ebD0C85bA14B0547668a198b9dbD".into(),
                    "x".into(),
                ],
                worker: "test_".to_string() + &i.to_string(),
            };

            let mut login_msg = serde_json::to_vec(&login).unwrap();

            login_msg.push(b'\n');
            w.write(&login_msg).await.unwrap();
            is_login = true;
            sleep(std::time::Duration::new(10, 0)).await;
        } else {
            let eth_get_work = ClientGetWork {
                id: 5,
                method: "eth_getWork".into(),
                params: vec![],
            };

            let mut eth_get_work_msg = serde_json::to_vec(&eth_get_work).unwrap();
            sleep(std::time::Duration::new(5, 0)).await;
            eth_get_work_msg.push(b'\n');
            w.write(&eth_get_work_msg).await.unwrap();

            //计算速率
            // let submit_hashrate = Client {
            //     id: 6,
            //     method: "eth_submitHashrate".into(),
            //     params: ["0x1111111".into(), "0x1111111".into()].to_vec(),
            //     worker: "test_91".into(),
            // };

            // let mut submit_hashrate_msg = serde_json::to_vec(&submit_hashrate).unwrap();

            // submit_hashrate_msg.push(b'\n');
            // w.write(&submit_hashrate_msg).await;

            sleep(std::time::Duration::new(20, 0)).await;
        }
    }
}

async fn server_to_client<R>(mut r: ReadHalf<R>) -> Result<()>
where
    R: AsyncRead,
{
    let mut job_queue = vec![];
    loop {
        let mut buf = vec![0; 1024];
        let len: usize =
            match tokio::time::timeout(std::time::Duration::new(60, 0), r.read(&mut buf)).await {
                Ok(got) => match got {
                    Ok(len) => len,
                    Err(e) => {
                        println!("读取错误 {}", e);
                        bail!("{}", e);
                    }
                },
                Err(e) => {
                    panic!("读取超时 {}", e);
                }
            };
        //let len = r.read(&mut buf).await?;
        if len == 0 {
            println!("客户端下线");
            return Ok(());
            //panic!("客户端下线");
        }
        let jobs = String::from_utf8(buf[0..len].to_vec()).unwrap();

        if let Ok(job) = serde_json::from_str::<Server>(&jobs) {
            println!("Got Message {:?}", job);
            if let Some(id) = job.result.get(0) {
                if job_queue.contains(id) {
                    println!("任务重复 : {}", id);
                } else {
                    job_queue.push(id.to_string());
                }
            }
        }
    }
}

pub async fn command_matches() -> Result<ArgMatches<'static>> {
    let matches = App::new(format!(
        "{}, 版本: {} commit: {} {}",
        crate_name!(),
        crate_version!(),
        version::commit_date(),
        version::short_sha()
    ))
    .version(crate_version!())
    //.author(crate_authors!("\n"))
    .about(crate_description!())
    .arg(
        Arg::with_name("phread")
            .short("p")
            .long("phread")
            .help("指定worker数量")
            .takes_value(true),
    )
    .arg(
        Arg::with_name("server")
            .short("s")
            .long("server")
            .help("指定服务器TCP端口")
            .takes_value(true),
    )
    .get_matches();
    Ok(matches)
}
#[tokio::main]
async fn main() -> Result<()> {
    let matches = command_matches().await?;
    let phread = matches.value_of("phread").unwrap_or_else(|| {
        println!("请正确填写本地监听端口 例如: -p 8888");
        std::process::exit(1);
    });

    let phread_int: i32 = phread.parse()?;

    let server = matches.value_of("server").unwrap_or_else(|| {
        println!("请正确填写服务器地址 例如: -s 8.0.0.0:8888");
        std::process::exit(1);
    });

    let mut i = 0;

    let mut v = vec![];
    while i <= phread_int {
        let client = TcpSocket::new_v4()?;
        let stream = client.connect(server.parse()?).await?;
        let (r, w) = split(stream);
        let a =
            tokio::spawn(
                async move { tokio::try_join!(client_to_server(w, i), server_to_client(r)) },
            );
        v.push(a);
        i += 1;
    }

    let outputs = future::join_all(v.into_iter().map(tokio::spawn)).await;

    println!("{:?}", outputs);

    Ok(())
}

//{\"id\":1,\"method\":\"eth_submitLogin\",\"params\":[\"0x98be5c44d574b96b320dffb0ccff116bda433b8e\",\"x\"],\"worker\":\"P0002\"}
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Client {
    pub id: u64,
    pub method: String,
    pub params: Vec<String>,
    pub worker: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientGetWork {
    pub id: u64,
    pub method: String,
    pub params: Vec<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientSubmitHashrate {
    pub id: u64,
    pub method: String,
    pub params: Vec<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Server {
    pub id: u64,
    pub result: Vec<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerError {
    pub id: u64,
    pub jsonrpc: String,
    pub error: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BinanceError {
    pub code: u64,
    pub message: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerJobsWichHeigh {
    pub id: u64,
    pub result: Vec<String>,
    pub jsonrpc: String,
    pub height: u64,
}

//币印 {"id":0,"jsonrpc":"2.0","result":["0x0d08e3f8adaf9b1cf365c3f380f1a0fa4b7dda99d12bb59d9ee8b10a1a1d8b91","0x1bccaca36bfde6e5a161cf470cbf74830d92e1013ee417c3e7c757acd34d8e08","0x000000007fffffffffffffffffffffffffffffffffffffffffffffffffffffff","00"], "height":13834471}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerId1 {
    pub id: u64,
    pub result: bool,
}
