from typing import List, Union
from pydantic import BaseModel, ValidationError
from flask import Flask, request, jsonify
from sqlalchemy.orm import relationship, sessionmaker
from sqlalchemy import create_engine, Column, Integer, String, Float, ForeignKey, Date, Time, Enum, Boolean
from sqlalchemy.ext.declarative import declarative_base
import datetime


# ...其他代码（例如引入的库、数据库连接等）...

class Candidate(db.Model):
    __tablename__ = 'candidate'
    id = Column(Integer, primary_key=True)
    # ...其他字段（例如内容、评论等）...

# ...其他模型定义（例如PostResponse、EditDialogRequest等）...

@app.route('/projects/<int:project_id>/generation_job/<int:id>/candidates', methods=['GET'])
def get_generation_job_candidates(project_id, id):
    candidates = db.query(Candidate).filter_by(generation_job_id=id).all()

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
        candidates = db.query(Candidate).filter_by(**{filter_field: filter_value}).all()
    elif filter_type == 'exists':
        if filter_value:
            candidates = db.query(Candidate).filter(Candidate.__table__.c[filter_field].isnot(None)).all()
        else:
            candidates = db.query(Candidate).filter(Candidate.__table__.c[filter_field].is_(None)).all()

    response_data = []
    for candidate in candidates:
        response_data.append(candidate.to_dict())  # 假设Candidate模型有一个将其转换为字典的方法

    return jsonify({"candidates": response_data})


@app.route('/projects/<int:project_id>/database/dialog/<int:id>', methods=['GET'])
def get_single_dialog(project_id, id):
    candidate = db.query(Candidate).filter_by(id=id).first()

    if not candidate:
        return jsonify({"error": "Candidate not found"}), 404

    return jsonify(candidate.to_dict())  # 假设Candidate模型有一个将其转换为字典的方法


@app.route('/projects/<int:project_id>/database/dialog/<int:id>', methods=['POST'])
def edit_dialog(project_id, id):
    try:
        request_data = EditDialogRequest.parse_raw(request.data)
    except ValidationError as e:
        return jsonify({"error": str(e)}), 400

    candidate = db.query(Candidate).filter_by(id=id).first()

    if not candidate:
        return jsonify({"error": "Candidate not found"}), 404

    field = request_data.field
    content = request_data.content

    setattr(candidate, field, content)
    db.commit()

    return jsonify(PostResponse())


if __name__ == '__main__':
    app.run()