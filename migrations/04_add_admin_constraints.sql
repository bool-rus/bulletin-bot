ALTER TABLE bot_admins RENAME TO _bot_admins;

CREATE TABLE bot_admins
(
    bot_id  INTEGER NOT NULL,
    user    INTEGER NOT NULL,
    username TEXT NOT NULL DEFAULT 'OWNER',

    FOREIGN KEY(bot_id) REFERENCES bots(id) ON DELETE CASCADE,
    UNIQUE(bot_id, user) ON CONFLICT REPLACE
);

INSERT INTO bot_admins SELECT * FROM _bot_admins;

DROP TABLE _bot_admins;
