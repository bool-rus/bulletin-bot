delete from bots where id in (
    select max(id) from bots group by token having count(*) > 1
);

CREATE UNIQUE INDEX token_unique_idx on bots (token);