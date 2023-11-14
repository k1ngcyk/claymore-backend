create table usage_v2(
    team_id uuid not null references team(team_id),
    project_id uuid not null references project(project_id),
    generator_id uuid not null references generator_v2(generator_id),
    user_id uuid not null references "user"("user_id"),
    token_count integer not null default 0,
    created_at timestamptz not null default now(),
    updated_at timestamptz
);

select trigger_updated_at('usage_v2');
