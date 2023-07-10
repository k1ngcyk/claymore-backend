from flask import request, jsonify
from pydantic import ValidationError

from setup import app, db
from models import EditDialogRequest, PostResponse, Dialog


@app.route('/projects/<int:project_id>/dialog', methods=['GET'])
def get_dialog(project_id):
    try:
        filter_field = request.args.get('field')
        filter_type = request.args.get('type')
        filter_value = request.args.get('value')

        if filter_type != 'value' or filter_type != 'exists':
            return jsonify({"error": 'wrong type'}), 400

    except ValidationError as e:
        return jsonify({"error": str(e)}), 400

    if filter_type == 'value':
        candidates = db.query(Dialog).filter_by(**{filter_field: filter_value}).all()
    elif filter_type == 'exists':
        if filter_value:
            candidates = db.query(Dialog).filter(Dialog.__table__.c[filter_field].isnot(None)).all()
        else:
            candidates = db.query(Dialog).filter(Dialog.__table__.c[filter_field].is_(None)).all()
    else:
        return jsonify({'error': 'unknown filter'}), 400
    response_data = []
    for candidate in candidates:
        response_data.append(candidate.model_dump())  # 假设Candidate模型有一个将其转换为字典的方法

    return jsonify({"candidates": response_data})


@app.route('/projects/<int:project_id>/dialog/<int:dialog_id>', methods=['GET'])
def get_single_dialog(project_id, dialog_id):
    candidate = db.query(Dialog).filter_by(project_id=project_id, id=dialog_id).first()

    if not candidate:
        return jsonify({"error": "Candidate not found"}), 404

    return jsonify(candidate.model_dump())  # 假设Candidate模型有一个将其转换为字典的方法


@app.route('/projects/<int:project_id>/dialog/<int:dialog_id>', methods=['POST'])
def edit_dialog(project_id, dialog_id):
    try:
        request_data = EditDialogRequest.model_validate(request.json)
    except ValidationError as e:
        return jsonify({"error": str(e)}), 400

    candidate = db.query(Dialog).filter_by(project_id=project_id, id=dialog_id).first()

    if not candidate:
        return jsonify({"error": "Candidate not found"}), 404

    field = request_data.field
    content = request_data.content

    setattr(candidate, field, content)
    db.commit()

    return jsonify(PostResponse().model_dump())