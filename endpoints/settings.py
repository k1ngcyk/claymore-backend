from typing import List, Union
from pydantic import ValidationError
from flask import request, jsonify
import datetime
from setup import app, db
from orms import Setting
from validators import AddSettingRequest, PostResponse, EditSettingRequest


@app.route('/projects/<int:project_id>/database/settings', methods=['GET'])
def get_settings(project_id):
    settings = db.query(Setting).filter_by(project_id=project_id).all()

    response_data = []
    for setting in settings:
        response_data.append({"key": setting.key, "value": setting.value})

    return jsonify({"settings": response_data})


@app.route('/projects/<int:project_id>/database/settings', methods=['POST'])
def add_setting(project_id):
    try:
        request_data = AddSettingRequest.parse_raw(request.data)
    except ValidationError as e:
        return jsonify({"error": str(e)}), 400

    new_setting = Setting(
        project_id=project_id,
        key=request_data.key,
        value=request_data.value,
        created_at=datetime.datetime.now(),
        updated_at=datetime.datetime.now()
    )

    db.add(new_setting)
    db.commit()

    return jsonify(PostResponse())


@app.route('/projects/<int:project_id>/database/settings/<int:id>', methods=['POST'])
def edit_setting(project_id, id):
    try:
        request_data = EditSettingRequest.parse_raw(request.data)
    except ValidationError as e:
        return jsonify({"error": str(e)}), 400

    setting = db.query(Setting).filter_by(project_id=project_id, id=id).first()

    if not setting:
        return jsonify({"error": "Setting not found"}), 404

    if request_data.type == 'edit':
        setattr(setting, request_data.field, request_data.content)
        setting.updated_at = datetime.datetime.now()
    elif request_data.type == 'delete':
        db.delete(setting)

    db.commit()

    return jsonify(PostResponse())