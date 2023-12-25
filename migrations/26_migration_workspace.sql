do $$
declare
    _user_id uuid;
    _workspace_id uuid;
begin
    for _user_id in select user_id from "user"
    loop
        if not exists(select 1 from workspace_v2 where owner_id = _user_id) then
            insert into workspace_v2 (workspace_name, owner_id, workspace_level, config_data)
            values ('Personal', _user_id, 0, '{"default": true}')
            returning workspace_id into _workspace_id;
            insert into workspace_member_v2 (workspace_id, user_id, user_level)
            values (_workspace_id, _user_id, 0);
        end if;
    end loop;
end $$;
