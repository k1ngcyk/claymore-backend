create table job(
    job_id uuid primary key default uuid_generate_v4(),
    job_name text not null,
    project_id uuid not null references project(project_id),
    generator_id uuid references generator(generator_id),
    target_count integer not null default 0,
    job_status integer not null default 0,
    prompt_chain jsonb,
    temperature float,
    word_count integer,
    created_at timestamptz not null default now(),
    updated_at timestamptz
);

select trigger_updated_at('job');
