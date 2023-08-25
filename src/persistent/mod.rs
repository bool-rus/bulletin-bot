
use std::sync::Arc;
use crossbeam::channel::{Sender, TryRecvError, Receiver};
use teloxide::types::{ChatId, UserId};


mod sqlite;

pub enum DBAction {
    AddListener(i64, Receiver<DBAction>),
    AddAdmin(i64, String),
    RemoveAdmin(i64),
    SetInfo(BotInfo),
    Ban{id: i64, name: String, cause: String},
    Unban(i64),
}

#[derive(Debug, Clone)]
pub struct BanInfo {
    pub name: String,
    pub cause: String,
}

pub struct BotInfo {
    pub username: String,
    pub channel_name: String,
    pub invite_link: Option<String>,
}

impl Default for BotInfo {
    fn default() -> Self {
        Self { username: "undefined".to_owned(), channel_name: "undefined".to_owned(), invite_link: None }
    }
}

#[derive(Debug, Clone)]
pub struct BulletinConfig {
    pub token: String,
    pub channel: ChatId,
    pub admins: Vec<(UserId, String)>,
    pub banned: Vec<(UserId, BanInfo)>,
    pub templates: Vec<(usize, String)>,
    pub tags: Vec<String>,
    pub flags: i32,
}

pub async fn worker() -> anyhow::Result<(Sender<DBAction>, Vec<(i64,BulletinConfig)>, Arc<Storage>)> {
    let storage = Storage::new(&crate::CONF.db_url).await?;
    let (s, r) = crossbeam::channel::unbounded();
    let configs = storage.all_configs().await?;
    let mut receivers = Vec::with_capacity(configs.len());

    let cloned_storage = storage.clone();
    tokio::spawn(async move {
        let storage = cloned_storage;
        loop {
            let mut sleep = true;
            match r.try_recv() {
                Ok(DBAction::AddListener(id, r)) => {
                    sleep = false;
                    receivers.push((r,id));
                },
                Err(TryRecvError::Disconnected) => {
                    log::info!("db worker stopped");
                    break;
                }
                _ => {},
            };
            let mut del_indexes = Vec::new();
            for (i, (r, bot_id)) in receivers.iter().enumerate() {
                match r.try_recv() {
                    Ok(action) => {
                        sleep = false;
                        use DBAction::*;
                        let result = match action {
                            AddListener(..) => {log::error!("Unexpected add listener"); Ok(())},
                            AddAdmin(admin_id, username) => storage.add_admin(*bot_id, admin_id, username).await,
                            RemoveAdmin(admin_id) => storage.remove_admin(*bot_id, admin_id).await,
                            SetInfo(bot_info) => storage.set_info(*bot_id, bot_info).await,
                            Ban { id, name , cause} => storage.ban(*bot_id, id, name, cause).await,
                            Unban(id) => storage.unban(*bot_id, id).await,
                        };
                        if let Some(e) = result.err() {
                            sleep = true;
                            //если мы получили какую-то ошибку, значит, БД временно недоступна
                            log::error!("error on update db: {}", e); //Кинем сообщение в лог
                            //TODO: положить сообщение в канал, чтобы потом его обработать
                        }
                    }
                    Err(TryRecvError::Disconnected) => del_indexes.push(i),
                    _ => {},
                }
            }
            del_indexes.iter().rev().for_each(|&i|{receivers.remove(i);});
            if sleep {
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        }
    });
    Ok((s, configs, storage))
}

pub use sqlite::Storage;

