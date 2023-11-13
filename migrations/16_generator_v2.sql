create table generator_v2(
    generator_id uuid primary key default uuid_generate_v4(),
    generator_name text not null,
    template_id uuid references template_v2(template_id),
    config_data jsonb not null,
    project_id uuid not null references project(project_id),
    created_at timestamptz not null default now(),
    updated_at timestamptz
);

select trigger_updated_at('generator_v2');
