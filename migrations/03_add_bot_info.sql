CREATE TABLE bot_info (
    bot_id INTEGER PRIMARY KEY NOT NULL,
    username TEXT NOT NULL,
    channel_name TEXT NOT NULL,
    FOREIGN KEY(bot_id) REFERENCES bots(id) ON DELETE CASCADE
);


