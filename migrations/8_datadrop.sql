create table datadrop(
    datadrop_id uuid primary key default uuid_generate_v4(),
    datadrop_name text not null,
    datadrop_content text not null,
    job_id uuid references job(job_id),
    project_id uuid not null references project(project_id),
    extra_data jsonb,
    created_at timestamptz not null default now(),
    updated_at timestamptz
);

select trigger_updated_at('datadrop');
