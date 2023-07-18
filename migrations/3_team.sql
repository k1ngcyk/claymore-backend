create table team(
    team_id uuid primary key default uuid_generate_v4(),
    team_name text not null,
    owner_id uuid not null references "user"(user_id),
    team_level integer not null default 0,
    created_at timestamptz not null default now(),
    updated_at timestamptz
);

select trigger_updated_at('team');
