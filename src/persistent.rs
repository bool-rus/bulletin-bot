
use std::sync::Arc;
use crossbeam::channel::{Sender, TryRecvError, Receiver};

use sqlx::{migrate::Migrator, SqlitePool};
use teloxide::types::{ChatId, UserId};

use crate::bots::bulletin::Config;

static MIGRATOR: Migrator = sqlx::migrate!();

pub enum DBAction {
    AddListener(i64, Receiver<DBAction>),
    AddAdmin(i64, String),
    RemoveAdmin(i64),
    SetInfo { name: String, channel_name: String },
}


#[derive(Debug, Clone)]
pub struct BulletinConfig {
    pub token: String,
    pub channel: ChatId,
    pub admins: Vec<BotAdmin>,
}

#[derive(Debug, Clone)]
pub struct BotAdmin {
    pub id: UserId,
    pub name: String,
}

pub async fn worker() -> (Sender<DBAction>, Vec<(i64,Config)>, Arc<Storage>) {
    let storage = Storage::new().await;
    let (s, r) = crossbeam::channel::unbounded();
    let configs = storage.all_configs().await;
    let mut receivers: Vec<_> = configs.iter().map(|(id, conf)|{
        (conf.receiver.clone(), *id)
    }).collect();

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
            for (i, (r, id)) in receivers.iter_mut().enumerate() {
                match r.try_recv() {
                    Ok(action) => {
                        sleep = false;
                        match action {
                            DBAction::AddListener(..) => log::error!("Unexpected add listener"),
                            DBAction::AddAdmin(admin_id, username) => storage.add_admin(*id, admin_id, username).await,
                            DBAction::RemoveAdmin(admin_id) => storage.remove_admin(*id, admin_id).await,
                            DBAction::SetInfo { name, channel_name } => storage.set_info(*id, name, channel_name).await,
                        };
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
    (s, configs, storage)
}


async fn make_pool() -> SqlitePool {
    let pool = SqlitePool::connect("sqlite://bulletin-configs.db").await.unwrap();
    MIGRATOR.run(&pool).await.unwrap();
    pool
}

pub struct Storage(SqlitePool);

impl Storage {
    async fn new() -> Arc<Self> {
        Arc::new(Self(make_pool().await))
    }
    pub async fn save_config(&self, cfg: Arc<Config>) -> i64 {
        let token = cfg.token.clone();
        let channel = cfg.channel.0;
        let mut conn = self.0.acquire().await.unwrap();
        sqlx::query!("insert into bots (token, channel) values (?1, ?2)", token, channel)
        .execute(&mut conn)
        .await.unwrap()
        .last_insert_rowid()
    }
    async fn all_configs(&self) -> Vec<(i64, Config)> {
        let mut conn = self.0.acquire().await.unwrap();
        let recs = sqlx::query!("select * from bots").fetch_all(&mut conn).await.unwrap();
        let mut res = Vec::with_capacity(recs.len());
        for r in recs {
            let id = r.id;
            let admins = sqlx::query!("select user, username from bot_admins where bot_id=?1", id)
            .fetch_all(&mut conn).await.unwrap()
            .iter().map(|r|(UserId(r.user as u64), r.username.clone())).collect();
            let conf = Config::new(r.token, ChatId(r.channel), admins);
            res.push((id,conf));
        }
        res
    }
    async fn add_admin(&self, bot_id: i64, admin_id: i64, username: String) {
        let mut conn = self.0.acquire().await.unwrap();
        sqlx::query!("insert into bot_admins values (?1, ?2, ?3)", bot_id, admin_id, username)
        .execute(&mut conn).await.unwrap();
    }
    async fn remove_admin(&self, bot_id: i64, admin_id: i64) {
        let mut conn = self.0.acquire().await.unwrap();
        sqlx::query!("delete from bot_admins where bot_id = ?1 and user = ?2", bot_id, admin_id)
        .execute(&mut conn).await.unwrap();
    }
    async fn set_info(&self, bot_id: i64, name: String, channel_name: String) {
        let mut conn = self.0.acquire().await.unwrap();
        sqlx::query!(
            "insert or replace into bot_info (bot_id, username, channel_name) values (?1, ?2, ?3)", 
            bot_id, name, channel_name
        ).execute(&mut conn).await.unwrap();
    }
    pub async fn get_bots(&self, admin_id: i64) -> Vec<(i64, String)> {
        let mut conn = self.0.acquire().await.unwrap();
        sqlx::query!(
            "select i.bot_id, i.username from bot_info as i join bot_admins as a on i.bot_id=a.bot_id where a.user=?1",
            admin_id
        ).fetch_all(&mut conn).await.unwrap().into_iter().map(|r|(r.bot_id, r.username)).collect()
    }
    pub async fn get_config(&self, bot_id: i64) -> Option<BulletinConfig> {
        let mut conn = self.0.acquire().await.unwrap();
        let mut rows = sqlx::query!(
            "select b.token, b.channel, a.user as user_id, a.username from bots b join bot_admins a on a.bot_id=b.id where b.id = ?1",
            bot_id
        ).fetch_all(&mut conn).await.unwrap().into_iter();
        let mut config: BulletinConfig = rows.next().map(|r|BulletinConfig {
            token: r.token, 
            channel: ChatId(r.channel), 
            admins: vec![BotAdmin{id: UserId(r.user_id as u64), name: r.username}]
        })?;
        rows.for_each(|r| {
            config.admins.push(BotAdmin { id: UserId(r.user_id as u64), name: r.username});
        });
        Some(config)
    }
}
