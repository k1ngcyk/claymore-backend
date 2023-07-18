create table generator(
    generator_id uuid primary key default uuid_generate_v4(),
    generator_name text not null,
    prompt_chain jsonb not null,
    model_name text not null,
    temperature float not null,
    word_count integer not null,
    project_id uuid not null references project(project_id),
    created_at timestamptz not null default now(),
    updated_at timestamptz
);

select trigger_updated_at('generator');
