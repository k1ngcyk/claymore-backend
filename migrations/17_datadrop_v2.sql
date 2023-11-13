create table datadrop_v2(
    datadrop_id uuid primary key default uuid_generate_v4(),
    datadrop_name text not null,
    datadrop_content text not null,
    generator_id uuid references generator_v2(generator_id),
    project_id uuid not null references project(project_id),
    extra_data jsonb,
    created_at timestamptz not null default now(),
    updated_at timestamptz
);

select trigger_updated_at('datadrop_v2');
