create table datastore_v2 (
    datastore_id uuid primary key default uuid_generate_v4(),
    datastore_name text not null,
    workspace_id uuid not null references workspace_v2(workspace_id),
    is_validated boolean not null default true,
    created_at timestamptz not null default now(),
    updated_at timestamptz
);

select trigger_updated_at('datastore_v2');

create table data_v2 (
    data_id uuid primary key default uuid_generate_v4(),
    datastore_id uuid references datastore_v2(datastore_id),
    module_id uuid references module_v2(module_id) on delete set null,
    data_module_type text default '',
    is_raw boolean not null default false,
    tags text default '',
    data_content text not null,
    extra_data jsonb,
    created_at timestamptz not null default now(),
    updated_at timestamptz
);

select trigger_updated_at('data_v2');
