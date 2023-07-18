create table team_member(
    team_id uuid not null references team(team_id),
    user_id uuid not null references "user"(user_id),
    user_level integer not null default 0,
    created_at timestamptz not null default now(),
    updated_at timestamptz,
    primary key (team_id, user_id)
);

select trigger_updated_at('team_member');
