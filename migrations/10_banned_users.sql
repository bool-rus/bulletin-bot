CREATE TABLE banned (
    bot_id INTEGER NOT NULL,
    user_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    cause TEXT NOT NULL,

    PRIMARY KEY(bot_id, user_id),
    FOREIGN KEY (bot_id) REFERENCES bots(id) ON DELETE CASCADE
);