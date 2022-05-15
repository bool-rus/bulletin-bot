pub use pers::Storage as DBStorage;

mod impls;
mod bots;

#[tokio::main]
async fn main() {
    init_logger();
    let storage = DBStorage::new().await;
    storage.all_configs().await.into_iter().for_each(|conf|bots::bulletin::start(conf.into()));
    bots::father::start(
        std::env::var("TELEGRAM_BOT_TOKEN").expect("need to set env variable TELEGRAM_BOT_TOKEN"), 
        storage
    );
    tokio::signal::ctrl_c().await.expect("Failed to listen for ^C");
    tokio::time::sleep(std::time::Duration::from_secs(10)).await;
}

fn init_logger() {
    use simplelog::*;
    TermLogger::init(LevelFilter::Info, Config::default(), TerminalMode::Mixed, ColorChoice::Auto).unwrap();
}

pub mod pers {

    use std::sync::Arc;

    use sqlx::{migrate::Migrator, SqlitePool, Sqlite};
    use teloxide::types::{ChatId, UserId};

    use crate::bots::bulletin::Config;

    static MIGRATOR: Migrator = sqlx::migrate!();

    async fn make_pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite://bulletin-configs.db").await.unwrap();
        MIGRATOR.run(&pool).await.unwrap();
        pool
    }

    pub struct Storage(SqlitePool);

    impl Storage {
        pub async fn new() -> Arc<Self> {
            Arc::new(Self(make_pool().await))
        }
        pub async fn create_config(&self, token: String, channel: i64, admin_id: i64) -> Config {
            let mut config = Config::new(token.clone(), ChatId(channel));
            config.add_admin(UserId(admin_id as u64));
            let mut conn = self.0.acquire().await.unwrap();
            let bot_id = sqlx::query!("insert into bots (token, channel) values (?1, ?2)", token, channel)
            .execute(&mut conn)
            .await.unwrap()
            .last_insert_rowid();
            sqlx::query!("insert into bot_admins values (?1, ?2)", bot_id, admin_id)
            .execute(&mut conn).await.unwrap();
            config
        }
        pub async fn all_configs(&self) -> Vec<Config> {
            let mut conn = self.0.acquire().await.unwrap();
            let recs = sqlx::query!("select * from bots").fetch_all(&mut conn).await.unwrap();
            let mut res = Vec::with_capacity(recs.len());
            for r in recs {
                let id = r.id;
                let mut conf = Config::new(r.token, ChatId(r.channel));
                let admins = sqlx::query!("select user from bot_admins where bot_id=?1", id)
                .fetch_all(&mut conn).await.unwrap();
                admins.iter().for_each(|r|conf.add_admin(UserId(r.user as u64)));
                res.push(conf);
            }
            res
        }
    }

}