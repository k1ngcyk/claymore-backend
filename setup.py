# 导入所需的库
from flask import Flask
from flask_sqlalchemy import SQLAlchemy

# 创建 Flask 应用实例
app = Flask(__name__)

# 配置数据库 URI
app.config['SQLALCHEMY_DATABASE_URI'] = 'sqlite:///example.db'
app.config['SQLALCHEMY_TRACK_MODIFICATIONS'] = False

# 初始化 SQLAlchemy
db = SQLAlchemy(app)
