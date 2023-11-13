create table openai(
    openai_id uuid primary key default uuid_generate_v4(),
    openai_key text not null,
    is_plus boolean not null default false,
    openai_status integer not null default 0,
    token_count integer not null default 0,
    created_at timestamptz not null default now(),
    updated_at timestamptz
);

select trigger_updated_at('openai');