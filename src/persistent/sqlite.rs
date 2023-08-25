use std::{sync::Arc, str::FromStr, ops::DerefMut};
use sqlx::{migrate::Migrator, SqlitePool, ConnectOptions, sqlite::SqliteConnectOptions};
use teloxide::types::{ChatId, UserId};

use super::{BulletinConfig, BotInfo, BanInfo};


static MIGRATOR: Migrator = sqlx::migrate!();
type Conn = sqlx::sqlite::SqliteConnection;


pub struct Storage(SqlitePool);

async fn make_pool(db_url: &str) -> anyhow::Result<SqlitePool> {
    let mut options = SqliteConnectOptions::from_str(db_url)?;
    options = options.disable_statement_logging();
    let pool = SqlitePool::connect_with(options).await?;
    MIGRATOR.run(&pool).await?;
    Ok(pool)
}

impl Storage {
    pub async fn new(db_url: &str) -> anyhow::Result<Arc<Self>> {
        Ok(Arc::new(Self(make_pool(db_url).await?)))
    }
    pub async fn close(&self) {
        log::info!("closing database connections...");
        self.0.close().await;
        log::info!("database connections closed!");
    }
    pub async fn save_config(&self, cfg: BulletinConfig) -> anyhow::Result<i64>  {
        let token = cfg.token.clone();
        let channel = cfg.channel.0;
        let mut conn = self.0.acquire().await?;
        let bot_id = sqlx::query!("insert into bots (token, channel) values (?1, ?2)", token, channel)
        .execute(&mut *conn)
        .await?
        .last_insert_rowid();
        for (admin_id, name) in cfg.admins {
            self.add_admin(bot_id, admin_id.0 as i64, name).await?;
        }
        Ok(bot_id)
    }
    pub async fn all_configs(&self) -> anyhow::Result<Vec<(i64, BulletinConfig)>> {
        let mut conn = self.0.acquire().await?;
        let recs = sqlx::query!("select * from bots").fetch_all(&mut *conn).await?;
        let mut res = Vec::with_capacity(recs.len());
        for r in recs {
            let id = r.id;
            let admins = get_admins(&mut *conn, id).await?;
            let templates = get_templates(&mut *conn, id).await?;
            let tags = get_tags(&mut *conn, id).await?;
            let banned = get_banned(&mut *conn, id).await?;
            let conf = BulletinConfig{
                token: r.token, 
                channel: ChatId(r.channel), 
                admins, banned, templates, tags, 
                flags: r.flags as i32,
            };
            res.push((id,conf));
        }
        Ok(res)
    }
    pub(super) async fn add_admin(&self, bot_id: i64, admin_id: i64, username: String) -> anyhow::Result<()>  {
        sqlx::query!("insert into bot_admins values (?1, ?2, ?3)", bot_id, admin_id, username)
            .execute(&self.0).await?;
        Ok(())
    }
    pub(super) async fn remove_admin(&self, bot_id: i64, admin_id: i64) -> anyhow::Result<()>  {
        sqlx::query!("delete from bot_admins where bot_id = ?1 and user = ?2", bot_id, admin_id)
            .execute(&self.0).await?;
        Ok(())
    }
    pub(super) async fn ban(&self, bot_id: i64, user_id: i64, name: String, cause: String) -> anyhow::Result<()>  {
        sqlx::query!("insert into banned (bot_id, user_id, name, cause) values (?1, ?2, ?3, ?4)", bot_id, user_id, name, cause)
            .execute(&self.0).await?;
        Ok(())
    }
    pub(super) async fn unban(&self, bot_id: i64, user_id: i64)  -> anyhow::Result<()> {
        sqlx::query!("delete from banned where bot_id=?1 and user_id=?2", bot_id, user_id)
            .execute(&self.0).await?;
        Ok(())
    }
    pub async fn get_info(&self, bot_id: i64) -> anyhow::Result<BotInfo>  {
        sqlx::query_as!(BotInfo, "select username, channel_name, invite_link from bot_info where bot_id=?1", bot_id)
            .fetch_one(&self.0).await.map_err(Into::into)
    }
    pub(super) async fn set_info(&self, bot_id: i64, bot_info: BotInfo) -> anyhow::Result<()> {
        let mut conn = self.0.acquire().await?;
        let BotInfo{username, channel_name, invite_link} = bot_info;
        sqlx::query!(
            "insert or replace into bot_info (bot_id, username, channel_name, invite_link) values (?1, ?2, ?3, ?4)", 
            bot_id, username, channel_name, invite_link
        ).execute(&mut *conn).await?;
        Ok(())
    } 
    pub async fn get_bots(&self, admin_id: i64) -> anyhow::Result<Vec<(i64, String)>> {
        let res = sqlx::query!(
            "select i.bot_id, i.username from bot_info as i join bot_admins as a on i.bot_id=a.bot_id where a.user=?1",
            admin_id
        ).fetch_all(&self.0).await?.into_iter().map(|r|(r.bot_id, r.username)).collect();
        Ok(res)
    }
    pub async fn get_config(&self, bot_id: i64) -> anyhow::Result<BulletinConfig> {
        let mut conn = self.0.acquire().await?;
        let bot = sqlx::query!(
            "select token, channel, flags from bots where id=?1",
            bot_id
        ).fetch_one(&mut *conn).await?;
        let admins = get_admins(&mut *conn, bot_id).await?;
        let templates = get_templates(&mut *conn, bot_id).await?;
        let tags = get_tags(&mut *conn, bot_id).await?;
        let banned = get_banned(&mut *conn, bot_id).await?;

        let config = BulletinConfig {
            token: bot.token, 
            channel: ChatId(bot.channel), 
            admins,
            banned,
            templates,
            tags,
            flags: bot.flags as i32,
        };
        Ok(config)
    }
    pub async fn delete_config(&self, bot_id: i64) -> anyhow::Result<()> {
        sqlx::query!("delete from bots where id=?1", bot_id).execute(&self.0).await?;
        Ok(())
    }
    pub async fn get_templates(&self, bot_id: i64) -> anyhow::Result<Vec<(usize, String)>> {
        get_templates(self.0.acquire().await?.deref_mut(), bot_id).await
    }
    pub async fn delete_template(&self, bot_id: i64, template_id: usize) -> anyhow::Result<()> {
        let template_id = template_id as u32;
        sqlx::query!("delete from bot_template where bot_id=?1 and text_id=?2", bot_id, template_id)
            .execute(&self.0).await?;
        Ok(())
    }
    pub async fn add_template(&self, bot_id: i64, template_id: usize, text: String) -> anyhow::Result<()> {
        let template_id = template_id as u32;
        sqlx::query!("insert into bot_template (bot_id, text_id, text) values (?1, ?2, ?3)",
            bot_id, template_id, text)
            .execute(&self.0).await?;
        Ok(())
    }
    pub async fn update_token(&self, bot_id: i64, token: String) -> anyhow::Result<()>{
        sqlx::query!("update bots set token = ?1 where id = ?2", token, bot_id)
            .execute(&self.0).await?;
        Ok(())
    }
    pub async fn add_tag(&self, bot_id: i64, name: String) -> anyhow::Result<()> {
        sqlx::query!("insert into tags (bot_id, name) values (?1, ?2)", bot_id, name)
            .execute(&self.0).await?;
        Ok(())
    }
    pub async fn delete_tag(&self, bot_id: i64, name: String) -> anyhow::Result<()> {
        sqlx::query!("delete from tags where bot_id = ?1 and name = ?2", bot_id, name)
            .execute(&self.0).await?;
        Ok(())
    }
    pub async fn get_tags(&self, bot_id: i64) -> anyhow::Result<Vec<String>> {
        get_tags(self.0.acquire().await?.deref_mut(), bot_id).await
    }
    pub async fn all_admins(&self) -> anyhow::Result<Vec<UserId>> {
        let res = sqlx::query!("select distinct user from bot_admins")
            .fetch_all(&self.0).await?
            .into_iter().map(|r|UserId(r.user as u64))
            .collect();
        Ok(res)
    }
    pub async fn update_flags(&self, bot_id: i64, flags: i32) -> anyhow::Result<()> {
        sqlx::query!("update bots set flags = ?1 where id = ?2", flags, bot_id)
            .execute(&self.0).await?;
        Ok(())
    }
}


async fn get_templates(conn: &mut Conn, bot_id: i64) -> anyhow::Result<Vec<(usize, String)>> {
    let res = sqlx::query!("select text_id, text from bot_template where bot_id=?1", bot_id)
        .fetch_all(conn).await?.into_iter()
        .map(|r|(r.text_id as usize, r.text)).collect();
    Ok(res)
}

async fn get_admins(conn: &mut Conn, bot_id: i64) -> anyhow::Result<Vec<(UserId, String)>> {
    let res = sqlx::query!("select user, username from bot_admins where bot_id=?1", bot_id)
        .fetch_all(conn).await?
        .into_iter().map(|r|(UserId(r.user as u64), r.username))
        .collect();
    Ok(res)
}

async fn get_banned(conn: &mut Conn, bot_id: i64) -> anyhow::Result<Vec<(UserId, BanInfo)>> {
    let res = sqlx::query!("select user_id, name, cause from banned where bot_id=?1", bot_id)
        .fetch_all(conn).await?
        .into_iter().map(|r|(UserId(r.user_id as u64), BanInfo{name: r.name, cause: r.cause}))
        .collect();
    Ok(res)
}

async fn get_tags(conn: &mut Conn, bot_id: i64) -> anyhow::Result<Vec<String>> {
    let res = sqlx::query!("select name from tags where bot_id = ?1", bot_id)
        .fetch_all(conn).await?
        .into_iter().map(|r|r.name)
        .collect();
    Ok(res)
}
