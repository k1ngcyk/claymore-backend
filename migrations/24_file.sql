create table files(
    file_id uuid primary key default uuid_generate_v4(),
    file_name text not null,
    file_path text not null,
    file_type text not null default 'text',
    md5 text not null,
    extra_data jsonb,
    created_at timestamptz not null default now(),
    updated_at timestamptz
);

select trigger_updated_at('files');

create table file_module(
    file_id uuid not null references files(file_id),
    module_id uuid not null references module_v2(module_id),
    finish_process boolean not null default false,
    extra_data jsonb,
    created_at timestamptz not null default now(),
    updated_at timestamptz,
    primary key (file_id, module_id)
);

select trigger_updated_at('file_module');
