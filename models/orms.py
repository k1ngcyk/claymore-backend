# 数据库 ORM
from datetime import datetime

import sqlalchemy

from setup import db
from sqlalchemy import Column, Integer, String, Date, Time, ForeignKey, Enum, Float, JSON, DateTime


class Generator(db.Model):
    __tablename__ = 'generator'
    id = Column(Integer, primary_key=True)
    project_id = Column(Integer, ForeignKey('project.id'))
    user_id = Column(Integer, ForeignKey('user.id'))
    created_at = Column(DateTime(timezone=True), default=datetime.utcnow)
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
    email = Column(String)
    name = Column(String)
    password = Column(String)


class GenerationJob(db.Model):
    __tablename__ = 'generation_job'
    id = Column(Integer, primary_key=True)
    project_id = Column(Integer, ForeignKey('project.id'))
    model_name = Column(String)
    temperature = Column(Float)
    tokens = Column(Integer)
    generator_id = Column(Integer, ForeignKey('generator.id'))
    name = Column(String)
    created_at = Column(Date)
    duration = Column(Time)
    task_id = Column(String) # Celery task id
    status = Column(Enum('Error', 'Running', 'Finished', 'Stopped', 'Waiting', name='GenerationJobStatus'), nullable=False)
    generated_count = Column(Integer)
    total_count = Column(Integer)
    variables = Column(JSON)

class Dialog(db.Model):
    __tablename__ = 'dialog'
    id = Column(Integer, primary_key=True)
    project_id = Column(Integer, ForeignKey('project.id'))
    content = Column(String)
    generation_job_id = Column(Integer, ForeignKey('generation_job.id'))
    source_type = Column(Enum('User', 'Generator', name='DialogSource'), nullable=False)
    source_id = Column(Integer)
    created_at = Column(Date)
    edited = Column(sqlalchemy.Boolean)
    attrs = Column(db.JSON) # for other metrics


class Feedback(db.Model):
    __tablename__ = 'feedback'
    project_id = Column(Integer, ForeignKey('project.id'), primary_key=True)
    user_id = Column(Integer, ForeignKey('user.id'), primary_key=True)
    dialog_id = Column(Integer, ForeignKey('dialog.id'), primary_key=True)
    created_at = Column(Date)
    comment = Column(String)
    quality = Column(Enum('Low', 'Medium', 'High', name='FeedbackQuality'))
    content = Column(db.JSON)


class Setting(db.Model):
    __tablename__ = 'setting'
    project_id = Column(Integer, ForeignKey('project.id'), primary_key=True)
    id = Column(Integer, primary_key=True)
    key = Column(String)
    value = Column(String)
    created_at = Column(Date)
    updated_at = Column(Date)



