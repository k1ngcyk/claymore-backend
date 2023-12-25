create table workspace_v2(
    workspace_id uuid primary key default uuid_generate_v4(),
    workspace_name text not null,
    owner_id uuid not null references "user"(user_id),
    workspace_level integer not null default 0,
    config_data jsonb not null,
    created_at timestamptz not null default now(),
    updated_at timestamptz
);

select trigger_updated_at('workspace_v2');

create table workspace_member_v2(
    workspace_id uuid not null references workspace_v2(workspace_id),
    user_id uuid not null references "user"(user_id),
    user_level integer not null default 0,
    created_at timestamptz not null default now(),
    updated_at timestamptz
);

select trigger_updated_at('workspace_member_v2');
