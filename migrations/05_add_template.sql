CREATE TABLE bot_template (
    bot_id INTEGER NOT NULL,
    text_id INTEGER NOT NULL,
    text TEXT NOT NULL,

    PRIMARY KEY (bot_id, text_id) ON CONFLICT REPLACE,
    FOREIGN KEY (bot_id) REFERENCES bots(id) ON DELETE CASCADE
);