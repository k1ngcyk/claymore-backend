# 导入所需的库
import os

from celery import Celery
from flask import Flask
from flask_sqlalchemy import SQLAlchemy
import openai
from dotenv import load_dotenv

load_dotenv()

openai.api_key = os.getenv('OPENAI_API_KEY')


class Config(object):
    SQLALCHEMY_DATABASE_URI = os.getenv('DATABASE_URL')
    SQLALCHEMY_TRACK_MODIFICATIONS = False


# 创建 Flask 应用实例
app = Flask('data-augmenter')
app.config.from_object(Config)

celery_app = Celery('tasks')
celery_app.config_from_object('celeryconfig')

# 初始化 SQLAlchemy
db = SQLAlchemy(app)
