
use std::sync::Arc;
use crossbeam::channel::{Sender, TryRecvError};

use sqlx::{migrate::Migrator, SqlitePool};
use teloxide::types::{ChatId, UserId};

use crate::bots::bulletin::Config;

static MIGRATOR: Migrator = sqlx::migrate!();

pub enum DBAction {
    CreateConfig(Arc<Config>),
    AddAdmin(i64, String),
    RemoveAdmin(i64),
    SetInfo { name: String, channel_name: String },
}

pub async fn worker() -> (Sender<DBAction>, Vec<Config>) {
    let storage = Storage::new().await;
    let (s, r) = crossbeam::channel::unbounded();
    let configs = storage.all_configs().await;
    let mut receivers: Vec<_> = configs.iter().map(|(id, conf)|{
        (conf.receiver.clone(), *id)
    }).collect();

    tokio::spawn(async move {
        loop {
            let mut sleep = true;
            match r.try_recv() {
                Ok(DBAction::CreateConfig(cfg)) => {
                    sleep = false;
                    let r = cfg.receiver.clone();
                    let id = storage.save_config(cfg).await;
                    receivers.push((r, id));
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
                            DBAction::CreateConfig(_) => log::error!("Unexpected create config"),
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
    (s, configs.into_iter().map(|(_,c)|c).collect())
}


async fn make_pool() -> SqlitePool {
    let pool = SqlitePool::connect("sqlite://bulletin-configs.db").await.unwrap();
    MIGRATOR.run(&pool).await.unwrap();
    pool
}

struct Storage(SqlitePool);

impl Storage {
    async fn new() -> Arc<Self> {
        Arc::new(Self(make_pool().await))
    }
    async fn save_config(&self, cfg: Arc<Config>) -> i64 {
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
            "insert or replace into bot_info (bot_id, username, channel_name) values (?1, ?2, ?3)", bot_id, name, channel_name
        ).execute(&mut conn).await.unwrap();
    }
}
