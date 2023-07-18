create table comment(
    comment_id uuid primary key default uuid_generate_v4(),
    user_id uuid not null references "user"(user_id),
    datadrop_id uuid not null references datadrop(datadrop_id),
    comment_content text not null,
    created_at timestamptz not null default now(),
    updated_at timestamptz
);

select trigger_updated_at('comment');
