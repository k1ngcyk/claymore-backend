from flask import request, jsonify
from pydantic import ValidationError

from setup import app, db
from models import GetDialogRequest, Candidate, EditDialogRequest, PostResponse, Dialog


@app.route('/projects/<int:project_id>/generation_job/<int:id>/candidates', methods=['GET'])
def get_generation_job_candidates(project_id, id):
    candidates = db.query(Dialog).filter_by(project_id=project_id, generation_job_id=id).all()

    response_data = []
    for candidate in candidates:
        response_data.append(candidate.to_dict())  # 假设Candidate模型有一个将其转换为字典的方法

    return jsonify({"candidates": response_data})


@app.route('/projects/<int:project_id>/dialog', methods=['GET'])
def get_dialog(project_id):
    try:
        request_data = GetDialogRequest.parse_raw(request.args.get('filter'))
    except ValidationError as e:
        return jsonify({"error": str(e)}), 400

    filter_type = request_data.filter.type
    filter_field = request_data.filter.field
    filter_value = request_data.filter.value

    if filter_type == 'value':
        candidates = db.query(Dialog).filter_by(**{filter_field: filter_value}).all()
    elif filter_type == 'exists':
        if filter_value:
            candidates = db.query(Dialog).filter(Candidate.__table__.c[filter_field].isnot(None)).all()
        else:
            candidates = db.query(Dialog).filter(Candidate.__table__.c[filter_field].is_(None)).all()

    response_data = []
    for candidate in candidates:
        response_data.append(candidate.to_dict())  # 假设Candidate模型有一个将其转换为字典的方法

    return jsonify({"candidates": response_data})


@app.route('/projects/<int:project_id>/database/dialog/<int:id>', methods=['GET'])
def get_single_dialog(project_id, id):
    candidate = db.query(Candidate).filter_by(project_id=project_id, id=id).first()

    if not candidate:
        return jsonify({"error": "Candidate not found"}), 404

    return jsonify(candidate.to_dict())  # 假设Candidate模型有一个将其转换为字典的方法


@app.route('/projects/<int:project_id>/database/dialog/<int:id>', methods=['POST'])
def edit_dialog(project_id, id):
    try:
        request_data = EditDialogRequest.parse_raw(request.data)
    except ValidationError as e:
        return jsonify({"error": str(e)}), 400

    candidate = db.query(Candidate).filter_by(project_id=project_id, id=id).first()

    if not candidate:
        return jsonify({"error": "Candidate not found"}), 404

    field = request_data.field
    content = request_data.content

    setattr(candidate, field, content)
    db.commit()

    return jsonify(PostResponse())