create table character(
    character_id uuid primary key default uuid_generate_v4(),
    character_name text not null,
    project_id uuid not null references project(project_id),
    settings jsonb not null,
    created_at timestamptz not null default now(),
    updated_at timestamptz
);

select trigger_updated_at('character');
