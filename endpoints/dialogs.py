from copy import deepcopy

from flask import request, jsonify
from pydantic import ValidationError
from sqlalchemy import func

from setup import app, db
from models.validators import EditDialogRequest, PostResponse, Status
from models.orms import Dialog, Feedback
from collections import Counter

@app.route('/projects/<int:project_id>/dialog', methods=['GET'])
def get_dialog(project_id):
    try:
        filter_field = request.args.get('field')
        filter_type = request.args.get('type')
        filter_value = request.args.get('value')

        if filter_type not in ['value', 'exists', 'all']:
            return jsonify({"error": 'wrong type'}), 400
    except ValidationError as e:
        return jsonify({"error": str(e)}), 400

    if filter_type == 'all':
        page = request.args.get('page', 1, type=int)
        per_page = request.args.get('per_page', 10, type=int)
        offset = (page - 1) * per_page

        dialogs = Dialog.query \
            .filter_by(project_id=project_id) \
            .order_by(Dialog.created_at.desc()) \
            .offset(offset) \
            .limit(per_page).all()
    elif filter_type == 'value':
        dialogs = db.query(Dialog) \
            .filter_by(project_id=project_id, **{filter_field: filter_value}) \
            .all()
    elif filter_type == 'exists':
        if filter_value:
            dialogs = db.query(Dialog).filter(Dialog.__table__.c[filter_field].isnot(None)).all()
        else:
            dialogs = db.query(Dialog).filter(Dialog.__table__.c[filter_field].is_(None)).all()
    else:
        return jsonify({'error': 'unknown filter'}), 400
    response_data = []
    for dialog in dialogs:
        response_data.append(dict(
            id=dialog.id,
            content=dialog.content,
            edited=dialog.edited,
            source_id=dialog.source_id,
        ))

    return jsonify({"dialogs": response_data})


@app.route('/projects/<int:project_id>/dialog/<int:dialog_id>', methods=['GET'])
def get_single_dialog(project_id, dialog_id):
    dialog = db.query(Dialog).filter_by(project_id=project_id, id=dialog_id).first()
    feedbacks = db.query(Feedback).filter_by(project_id=project_id, dialog_id=dialog_id).all()
    comments = [feedback.comment for feedback in feedbacks if feedback.comment]

    if not dialog:
        return jsonify({"error": "Dialog not found"}), 404
    quality_values = {
        'Low': 1,
        'Medium': 2,
        'High': 3
    }
    qualities = [item.quality for item in feedbacks]
    quality_counts = Counter(qualities)
    avg = sum([quality_values[quality] for quality in qualities]) / len(feedbacks)
    return jsonify(dict(
        content=dialog.content,
        quality=avg,
        quality_counts=quality_counts,
        comments=comments,
        edited=dialog.edited,
        unmarked=dialog.average_quality - 0 < 1e-8,
    )), 200


@app.route('/projects/<int:project_id>/dialog/<int:dialog_id>', methods=['POST'])
def edit_dialog(project_id, dialog_id):
    try:
        request_data = EditDialogRequest.model_validate(request.json)
    except ValidationError as e:
        return jsonify({"error": str(e)}), 400

    candidate = db.query(Dialog).filter_by(project_id=project_id, id=dialog_id).first()

    if not candidate:
        return jsonify({"error": "Dialog not found"}), 404
    field = request_data.field
    content = request_data.content

    if request_data.field in candidate.attrs:
        new_attrs = deepcopy(candidate.attrs)
        new_attrs[field] = content
        candidate.attrs = new_attrs
    db.commit()

    return jsonify(PostResponse(status=Status.success, message="Dialog added").model_dump())
