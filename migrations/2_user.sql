create table "user"(
    user_id uuid primary key default uuid_generate_v4(),
    user_name text collate "case_insensitive" unique not null,
    email text collate "case_insensitive" unique not null,
    password_hash text not null,
    created_at timestamptz not null default now(),
    updated_at timestamptz
);

select trigger_updated_at('"user"');
