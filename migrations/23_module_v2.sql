create table module_v2(
    module_id uuid primary key default uuid_generate_v4(),
    module_name text not null,
    template_id uuid references template_v2(template_id),
    config_data jsonb not null,
    workspace_id uuid not null references workspace_v2(workspace_id),
    module_category text not null default '',
    created_at timestamptz not null default now(),
    updated_at timestamptz
);

select trigger_updated_at('module_v2');

create table job_v2(
    job_id uuid primary key default uuid_generate_v4(),
    module_id uuid references module_v2(module_id),
    config_data jsonb not null,
    workspace_id uuid not null references workspace_v2(workspace_id),
    target_count integer not null default 0,
    job_status integer not null default 0,
    created_at timestamptz not null default now(),
    updated_at timestamptz
);

select trigger_updated_at('job_v2');

create table candidate_v2(
    candidate_id uuid primary key default uuid_generate_v4(),
    content text not null,
    module_id uuid references module_v2(module_id),
    job_id uuid references job_v2(job_id),
    job_status_group_id uuid not null,
    extra_data jsonb,
    created_at timestamptz not null default now(),
    updated_at timestamptz
);

select trigger_updated_at('candidate_v2');

create table metric_v2(
    workspace_id uuid references workspace_v2(workspace_id),
    user_id uuid references "user"(user_id),
    module_id uuid references module_v2(module_id),
    job_id uuid references job_v2(job_id),
    token_count integer not null default 0,
    word_count integer not null default 0,
    extra_data jsonb,
    created_at timestamptz not null default now(),
    updated_at timestamptz
);

select trigger_updated_at('metric_v2');
