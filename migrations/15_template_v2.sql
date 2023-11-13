create table template_v2(
    template_id uuid primary key default uuid_generate_v4(),
    template_name text not null,
    template_icon text not null,
    template_description text not null,
    template_data jsonb not null,
    created_at timestamptz not null default now(),
    updated_at timestamptz
);

select trigger_updated_at('template_v2');
