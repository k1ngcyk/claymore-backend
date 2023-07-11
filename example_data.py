from datetime import datetime
from setup import db, app
from models.orms import Generator, Preset, Project, ProjectUser, User, Dialog, Feedback, Setting, GenerationJob
from datetime import time

with app.app_context():
    db.drop_all()
    db.create_all()
    # 创建一个新用户
    user = User(
        email="test@example.com",
        name="Test User",
        password="password123"
    )
    db.session.add(user)
    db.session.commit()

    # 创建一个新项目
    project = Project(name="Test Project")
    db.session.add(project)
    db.session.commit()

    # 将用户添加到项目
    project_user = ProjectUser(
        project_id=project.id,
        user_id=user.id,
        entered_at=datetime.now()
    )
    db.session.add(project_user)
    db.session.commit()

    # 创建一个生成器
    generator = Generator(
        project_id=project.id,
        user_id=user.id,
        created_at=datetime.now(),
        name="Dialog Writer",
        content=
        [
            'List three kinds of emotions. Do not explain.',
            'Write dialog based on the emotions: ^^.'
        ]
    )
    db.session.add(generator)
    db.session.commit()

    # 创建一个预设
    preset = Preset(
        project_id=project.id,
        generator_id=generator.id
    )
    db.session.add(preset)
    db.session.commit()

    # 创建一个设置
    # setting = Setting(
    #     project_id=project.id,
    #     key="test_setting",
    #     value="test_value",
    #     created_at=datetime.now(),
    #     updated_at=datetime.now()
    # )
    # db.session.add(setting)
    # db.session.commit()

    # 创建一个生成任务
    generation_job = GenerationJob(
        project_id=project.id,
        model_name="gpt-3.5-turbo",
        temperature=0.8,
        tokens=1024,
        generator_id=generator.id,
        name="Test Generation Job",
        created_at=datetime.now(),
        duration=time(),
        task_id="12345",
        status="Waiting",
        generated_count=0,
        total_count=10,
        variables={"example": "data"}
    )
    db.session.add(generation_job)
    db.session.commit()

    # 创建一个对话
    dialog = Dialog(
        project_id=project.id,
        content="Test dialog content",
        generation_job_id=generation_job.id,
        source_type='Generator',
        source_id=generator.id,
        created_at=datetime.now(),
        edited=False,
        attrs={"example": "data"}
    )
    db.session.add(dialog)
    db.session.commit()

    # 创建一个反馈
    feedback = Feedback(
        project_id=project.id,
        user_id=user.id,
        dialog_id=dialog.id,
        created_at=datetime.now(),
        comment="Test feedback comment",
        quality="Medium",
        content={"extra_metric": "value"}
    )
    db.session.add(feedback)
    db.session.commit()

    print("Test data added successfully.")
