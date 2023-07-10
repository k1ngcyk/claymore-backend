# 导入所需的库
from celery import Celery
from flask import Flask
from flask_sqlalchemy import SQLAlchemy

# 创建 Flask 应用实例
app = Flask('data-augmenter')

# 配置数据库 URI
app.config['SQLALCHEMY_DATABASE_URI'] = 'sqlite:///example.db'
app.config['SQLALCHEMY_TRACK_MODIFICATIONS'] = False

celery_app = Celery('tasks')
celery_app.config_from_object('celeryconfig')

# 初始化 SQLAlchemy
db = SQLAlchemy(app)
