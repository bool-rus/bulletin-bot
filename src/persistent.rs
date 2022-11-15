
use std::sync::Arc;
use crossbeam::channel::{Sender, TryRecvError, Receiver};

use sqlx::{migrate::Migrator, SqlitePool, Sqlite, ConnectOptions, sqlite::SqliteConnectOptions};
use teloxide::types::{ChatId, UserId};

static MIGRATOR: Migrator = sqlx::migrate!();
type Conn = sqlx::pool::PoolConnection<Sqlite>;

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
    pub admins: Vec<(UserId, String)>,
    pub templates: Vec<(usize, String)>,
    pub tags: Vec<String>,
}

pub async fn worker() -> (Sender<DBAction>, Vec<(i64,BulletinConfig)>, Arc<Storage>) {
    let storage = Storage::new().await;
    let (s, r) = crossbeam::channel::unbounded();
    let configs = storage.all_configs().await;
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
    let mut options = SqliteConnectOptions::new().filename("bulletin-configs.db");
    options.disable_statement_logging();
    let pool = SqlitePool::connect_with(options).await.unwrap();
    MIGRATOR.run(&pool).await.unwrap();
    pool
}

pub struct Storage(SqlitePool);

impl Storage {
    async fn new() -> Arc<Self> {
        Arc::new(Self(make_pool().await))
    }
    pub async fn close(&self) {
        log::info!("closing database connections...");
        self.0.close().await;
        log::info!("database connections closed!");
    }
    pub async fn save_config(&self, cfg: BulletinConfig) -> i64 {
        let token = cfg.token.clone();
        let channel = cfg.channel.0;
        let mut conn = self.0.acquire().await.unwrap();
        let bot_id = sqlx::query!("insert into bots (token, channel) values (?1, ?2)", token, channel)
        .execute(&mut conn)
        .await.unwrap()
        .last_insert_rowid();
        for (admin_id, name) in cfg.admins {
            self.add_admin(bot_id, admin_id.0 as i64, name).await;
        }
        bot_id
    }
    async fn all_configs(&self) -> Vec<(i64, BulletinConfig)> {
        let mut conn = self.0.acquire().await.unwrap();
        let recs = sqlx::query!("select * from bots").fetch_all(&mut conn).await.unwrap();
        let mut res = Vec::with_capacity(recs.len());
        for r in recs {
            let id = r.id;
            let admins = get_admins(&mut conn, id).await;
            let templates = get_templates(&mut conn, id).await;
            let tags = get_tags(&mut conn, id).await;
            let conf = BulletinConfig{token: r.token, channel: ChatId(r.channel), admins, templates, tags};
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
        let bot = sqlx::query!(
            "select token, channel from bots where id=?1",
            bot_id
        ).fetch_optional(&mut conn).await.unwrap()?;
        let admins = get_admins(&mut conn, bot_id).await;
        let templates = get_templates(&mut conn, bot_id).await;
        let tags = get_tags(&mut conn, bot_id).await;

        let config = BulletinConfig {
            token: bot.token, 
            channel: ChatId(bot.channel), 
            admins,
            templates,
            tags,
        };
        Some(config)
    }
    pub async fn delete_config(&self, bot_id: i64) {
        let mut conn = self.0.acquire().await.unwrap();
        sqlx::query!("delete from bots where id=?1", bot_id).execute(&mut conn).await.unwrap();
    }
    pub async fn get_templates(&self, bot_id: i64) -> Vec<(usize, String)> {
        get_templates(&mut self.0.acquire().await.unwrap(), bot_id).await
    }
    pub async fn delete_template(&self, bot_id: i64, template_id: usize) {
        let template_id = template_id as u32;
        sqlx::query!("delete from bot_template where bot_id=?1 and text_id=?2", bot_id, template_id)
            .execute(&mut self.0.acquire().await.unwrap()).await.unwrap();
    }
    pub async fn add_template(&self, bot_id: i64, template_id: usize, text: String) {
        let template_id = template_id as u32;
        sqlx::query!("insert into bot_template (bot_id, text_id, text) values (?1, ?2, ?3)",
            bot_id, template_id, text)
            .execute(&mut self.0.acquire().await.unwrap()).await.unwrap();
    }
    pub async fn update_token(&self, bot_id: i64, token: String) {
        sqlx::query!("update bots set token = ?1 where id = ?2", token, bot_id)
            .execute(&mut self.0.acquire().await.unwrap()).await.unwrap();
    }
    pub async fn add_tag(&self, bot_id: i64, name: String) {
        sqlx::query!("insert into tags (bot_id, name) values (?1, ?2)", bot_id, name)
            .execute(&mut self.0.acquire().await.unwrap()).await.unwrap();
    }
    pub async fn delete_tag(&self, bot_id: i64, name: String) {
        sqlx::query!("delete from tags where bot_id = ?1 and name = ?2", bot_id, name)
            .execute(&mut self.0.acquire().await.unwrap()).await.unwrap();
    }
    pub async fn get_tags(&self, bot_id: i64) -> Vec<String> {
        get_tags(&mut self.0.acquire().await.unwrap(), bot_id).await
    }
}


async fn get_templates(conn: &mut Conn, bot_id: i64) -> Vec<(usize, String)> {
    sqlx::query!("select text_id, text from bot_template where bot_id=?1", bot_id)
        .fetch_all(conn).await.unwrap().into_iter()
        .map(|r|(r.text_id as usize, r.text)).collect()
}

async fn get_admins(conn: &mut Conn, bot_id: i64) -> Vec<(UserId, String)> {
    sqlx::query!("select user, username from bot_admins where bot_id=?1", bot_id)
        .fetch_all(conn).await.unwrap()
        .into_iter().map(|r|(UserId(r.user as u64), r.username))
        .collect()
}

async fn get_tags(conn: &mut Conn, bot_id: i64) -> Vec<String> {
    sqlx::query!("select name from tags where bot_id = ?1", bot_id)
        .fetch_all(conn).await.unwrap()
        .into_iter().map(|r|r.name)
        .collect()
}
