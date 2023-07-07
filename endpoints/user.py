from flask import request, jsonify
from setup import app, db
from models import User
from validators import PostResponse, LoginRequest, RegisterRequest


@app.route('/api/login', methods=['POST'])
def login():
    req_data = request.json
    try:
        login_request = LoginRequest(**req_data)
    except ValueError as e:
        return jsonify(PostResponse(status="error", message=str(e))), 400

    user = User.query.filter_by(name=login_request.name).first()

    if user and user.password == login_request.password:
        return jsonify(PostResponse(status="success", message="登录成功"))
    else:
        return jsonify(PostResponse(status="error", message="用户名或密码错误")), 401

@app.route('/api/register', methods=['POST'])
def register():
    req_data = request.json
    try:
        register_request = RegisterRequest(**req_data)
    except ValueError as e:
        return jsonify(PostResponse(status="error", message=str(e))), 400

    existing_user = User.query.filter_by(name=register_request.name).first()
    if existing_user:
        return jsonify(PostResponse(status="error", message="用户名已存在")), 409

    new_user = User(name=register_request.name, password=register_request.password)
    db.session.add(new_user)
    db.session.commit()

    return jsonify(PostResponse(status="success", message="注册成功"))