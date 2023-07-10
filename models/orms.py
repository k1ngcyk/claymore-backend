# 数据库 ORM

from setup import db
from sqlalchemy import Column, Integer, String, Date, Time, ForeignKey, Enum, Float, JSON


class Generator(db.Model):
    __tablename__ = 'generator'
    id = Column(Integer, primary_key=True)
    project_id = Column(Integer, ForeignKey('project.id'))
    user_id = Column(Integer, ForeignKey('user.id'))
    created_at = Column(Date)
    name = Column(String)
    content = Column(db.JSON)


class Preset(db.Model):
    __tablename__ = 'preset'
    id = Column(Integer, primary_key=True)
    project_id = Column(Integer, ForeignKey('project.id'))
    generator_id = Column(Integer, ForeignKey('generator.id'))


class Project(db.Model):
    __tablename__ = 'project'
    id = Column(Integer, primary_key=True)
    name = Column(String)


class ProjectUser(db.Model):
    __tablename__ = 'project_user'
    project_id = Column(Integer, ForeignKey('project.id'), primary_key=True)
    user_id = Column(Integer, ForeignKey('user.id'), primary_key=True)
    entered_at = Column(Date)


class User(db.Model):
    __tablename__ = 'user'
    id = Column(Integer, primary_key=True)
    name = Column(String)
    password = Column(String)


class Dialog(db.Model):
    __tablename__ = 'dialog'
    id = Column(Integer, primary_key=True)
    project_id = Column(Integer, ForeignKey('project.id'))
    content = Column(String)
    source_type = Column(Enum('User', 'Generator'), nullable=False)
    source_id = Column(Integer)
    status = Column(Enum('Testing', 'Candidate', 'Canon', 'Removed'), nullable=False)
    created_at = Column(Date)
    attrs = Column(db.JSON)


class Feedback(db.Model):
    __tablename__ = 'feedback'
    project_id = Column(Integer, ForeignKey('project.id'), primary_key=True)
    user_id = Column(Integer, ForeignKey('user.id'), primary_key=True)
    dialog_id = Column(Integer, ForeignKey('dialog.id'), primary_key=True)
    created_at = Column(Date)
    comment = Column(String)
    content = Column(db.JSON)


class Setting(db.Model):
    __tablename__ = 'setting'
    project_id = Column(Integer, ForeignKey('project.id'), primary_key=True)
    id = Column(Integer, primary_key=True)
    key = Column(String)
    value = Column(String)
    created_at = Column(Date)
    updated_at = Column(Date)


class GenerationJob(db.Model):
    __tablename__ = 'generation_job'
    id = Column(Integer, primary_key=True)
    project_id = Column(Integer, ForeignKey('project.id'), primary_key=True)
    model_name = Column(String)
    temperature = Column(Float)
    tokens = Column(Integer)
    generator_id = Column(Integer, ForeignKey('generator.id'))
    name = Column(String)
    created_at = Column(Date)
    duration = Column(Time)
    task_id = Column(String) # Celery task id
    status = Column(Enum('Error', 'Running', 'Finished', 'Stopped', 'Waiting'), nullable=False)
    generated_count = Column(Integer)
    total_count = Column(Integer)
    variables = Column(JSON)


