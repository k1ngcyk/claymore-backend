create table feedback(
    feedback_id uuid primary key default uuid_generate_v4(),
    user_id uuid not null references "user"(user_id),
    datadrop_id uuid not null references datadrop(datadrop_id),
    feedback_content jsonb not null,
    created_at timestamptz not null default now(),
    updated_at timestamptz,
    unique(user_id, datadrop_id)
);

select trigger_updated_at('feedback');
