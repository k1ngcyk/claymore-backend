alter table job
    drop constraint job_generator_id_fkey;

alter table job
    add foreign key (generator_id) references generator
        on delete set null;

alter table job
    drop constraint job_project_id_fkey;

alter table job
    add foreign key (project_id) references project
        on delete cascade;

alter table generator
    drop constraint generator_project_id_fkey;

alter table generator
    add foreign key (project_id) references project
        on delete cascade;

alter table datadrop
    drop constraint datadrop_job_id_fkey;

alter table datadrop
    add foreign key (job_id) references job
        on delete set null;

alter table datadrop
    drop constraint datadrop_project_id_fkey;

alter table datadrop
    add foreign key (project_id) references project
        on delete cascade;

alter table comment
    drop constraint comment_user_id_fkey;

alter table comment
    add foreign key (user_id) references "user"
        on delete set null;

alter table comment
    drop constraint comment_datadrop_id_fkey;

alter table comment
    add foreign key (datadrop_id) references datadrop
        on delete cascade;

alter table character
    drop constraint character_project_id_fkey;

alter table character
    add foreign key (project_id) references project
        on delete cascade;

