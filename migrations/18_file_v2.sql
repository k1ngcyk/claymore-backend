create table file_v2(
    file_id uuid primary key default uuid_generate_v4(),
    file_name text not null,
    file_path text not null,
    file_type text not null,
    md5 text not null,
    team_id uuid not null references team(team_id),
    created_at timestamptz not null default now(),
    updated_at timestamptz
);

select trigger_updated_at('file_v2');

create table file_generator_v2(
    file_id uuid not null references file_v2(file_id),
    generator_id uuid not null references generator_v2(generator_id),
    finish_process boolean not null default false,
    created_at timestamptz not null default now(),
    updated_at timestamptz,
    primary key (file_id, generator_id)
);

select trigger_updated_at('file_generator_v2');
