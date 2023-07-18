create table project(
    project_id uuid primary key default uuid_generate_v4(),
    project_name text not null,
    team_id uuid not null references team(team_id),
    created_at timestamptz not null default now(),
    updated_at timestamptz
);

select trigger_updated_at('project');
