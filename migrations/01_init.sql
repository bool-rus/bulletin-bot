CREATE TABLE IF NOT EXISTS bots (
    id      INTEGER PRIMARY KEY NOT NULL,
    token   TEXT                NOT NULL,
    channel INTEGER             NOT NULL
);

CREATE TABLE IF NOT EXISTS bot_admins (
    bot_id  INTEGER NOT NULL,
    user    INTEGER NOT NULL
);


