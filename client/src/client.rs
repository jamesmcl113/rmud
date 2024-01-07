use std::sync::Arc;

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, BufReader},
    net::{tcp::OwnedWriteHalf, TcpStream},
    sync::{mpsc, Mutex},
};

use model::UserAction;

pub struct RawTask {
    pub req: UserAction,
}

pub struct TaskSpawner {
    send: mpsc::Sender<RawTask>,
}

async fn handle_task_raw(socket: Arc<Mutex<OwnedWriteHalf>>, task: RawTask) {
    let mut socket = socket.lock().await;
    let req_bytes: Vec<u8> = task.req.into();
    socket.write_all(&req_bytes).await.unwrap();
    socket.flush().await.unwrap();
}

impl TaskSpawner {
    pub fn new() -> (TaskSpawner, mpsc::UnboundedReceiver<model::Response>) {
        let (send, mut recv) = mpsc::channel::<RawTask>(100);
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        let (tx, rx) = mpsc::unbounded_channel();

        let socket = rt.block_on(TcpStream::connect("127.0.0.1:8080")).unwrap();
        let (reader, writer) = socket.into_split();
        let mut reader = BufReader::new(reader);
        let writer = Arc::new(Mutex::new(writer));

        std::thread::spawn(move || {
            rt.block_on(async move {
                loop {
                    let mut buf = [0u8; 1024];
                    tokio::select! {
                        res = reader.read(&mut buf) => {
                            match res {
                                Ok(0) => todo!(),
                                Ok(n) => {
                                    //let tx = tx.clone();
                                    let res: model::Response = bincode::deserialize(&buf[0..n]).unwrap();
                                    tx.send(res).unwrap();
                                },
                                Err(_) => todo!(),
                            }
                        }
                        task = recv.recv() => {
                            if let Some(task) = task {
                                tokio::spawn(handle_task_raw(writer.clone(), task));
                            }
                        }
                    }
                }
            });
        });

        (TaskSpawner { send }, rx)
    }

    pub fn spawn_task(&self, task: RawTask) {
        match self.send.blocking_send(task) {
            Ok(_) => {}
            Err(_) => panic!("The shared runtime has shut down."),
        }
    }
}
