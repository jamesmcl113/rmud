use std::sync::Arc;

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, BufReader},
    net::{tcp::OwnedWriteHalf, TcpStream},
    sync::{mpsc, Mutex},
};

use model::Request;

enum TaskConfig {
    Command(model::UserActions),
    Chat(String),
}

pub struct Task {
    config: TaskConfig,
}

impl Task {
    pub fn send_chat(msg: &str) -> Task {
        Task {
            config: TaskConfig::Chat(msg.to_string()),
        }
    }

    pub fn send_command(cmd: &str, args: &[&str]) -> Result<Task, String> {
        let config = match cmd {
            "who" => TaskConfig::Command(model::UserActions::GetUsers),
            "rooms" | "rs" => TaskConfig::Command(model::UserActions::GetRooms),
            "pm" => {
                let to = args.get(0).ok_or("Expected a username.")?;
                let msg = &args[1..].join(" ");

                TaskConfig::Command(model::UserActions::PrivateMessage {
                    to: to.to_string(),
                    msg: msg.to_string(),
                })
            }
            "mv" => {
                let new_room = args.get(0).ok_or("Expected a room name.")?;
                TaskConfig::Command(model::UserActions::MoveRoom {
                    room_name: new_room.to_string(),
                })
            }
            _ => {
                return Err(format!("Unkown command: {cmd}"));
            }
        };
        Ok(Task { config })
    }
}

pub struct TaskSpawner {
    send: mpsc::Sender<Task>,
}

async fn handle_task(socket: Arc<Mutex<OwnedWriteHalf>>, task: Task) {
    let mut socket = socket.lock().await;
    let bytes: Vec<u8> = match task.config {
        TaskConfig::Chat(msg) => Request::UserMessage(msg).into(),
        TaskConfig::Command(cmd) => Request::UserAction(cmd).into(),
    };
    socket.write_all(&bytes).await.unwrap();
    socket.flush().await.unwrap();
}

impl TaskSpawner {
    pub fn new() -> (TaskSpawner, mpsc::Receiver<model::Response>) {
        let (send, mut recv) = mpsc::channel::<Task>(100);
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        let (tx, rx) = mpsc::channel(100);

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
                                    let tx = tx.clone();
                                    let res: model::Response = bincode::deserialize(&buf[0..n]).unwrap();
                                    tx.send(res).await.unwrap();
                                },
                                Err(_) => todo!(),
                            }
                        }
                        task = recv.recv() => {
                            if let Some(task) = task {
                                tokio::spawn(handle_task(writer.clone(), task));
                            }
                        }
                    }
                }
            });
        });

        (TaskSpawner { send }, rx)
    }

    pub fn spawn_task(&self, task: Task) {
        match self.send.blocking_send(task) {
            Ok(_) => {}
            Err(_) => panic!("The shared runtime has shut down."),
        }
    }
}
