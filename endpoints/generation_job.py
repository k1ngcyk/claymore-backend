import json
from datetime import timedelta

from flask import request, jsonify
from jsons import ValidationError
from setup import app, db, celery_app
from models.orms import Generator, Dialog, GenerationJob
from models.validators import (
    AddGeneratorRequest,
    PostResponse,
    CreateGenerationJobRequest,
    GenerationJobResponse,
    GenerationJobActionRequest,
    JobDetailResponse,
    Status,
    GenerationJobListResponse)
from tasks.generation import generate_dialogs


@app.route('/projects/<int:project_id>/generator', methods=['POST'])
def add_generator(project_id):
    req_data = request.json
    try:
        add_generator_request = AddGeneratorRequest(**req_data)
    except ValueError as e:
        return jsonify(PostResponse(status=Status.failure, message=str(e)).model_dump()), 400

    new_generator = Generator(name=add_generator_request.name,
                              content=add_generator_request.content,
                              user_id=add_generator_request.user_id,
                              project_id=project_id)
    db.session.add(new_generator)
    db.session.commit()

    return jsonify(PostResponse(status=Status.success, message="Added Generator").model_dump())


@app.route('/projects/<int:project_id>/generation_job/<int:id>/candidates', methods=['GET'])
def get_generation_job_candidates(project_id, id):
    candidates = Dialog.query.filter_by(project_id=project_id, generation_job_id=id).all()

    response_data = []
    for candidate in candidates:
        response_data.append(candidate.model_dump())

    return jsonify({"candidates": response_data})


@app.route('/projects/<int:project_id>/generation_job', methods=['POST'])
def create_generation_job(project_id):
    req_data = request.json
    try:
        create_generation_job_request = CreateGenerationJobRequest(**req_data)
    except ValueError as e:
        return jsonify(PostResponse(status=Status.failure, message=str(e)).model_dump()), 400

    generator = Generator.query.get(create_generation_job_request.generator_id)

    if not generator:
        return jsonify(PostResponse(status=Status.failure, message="生成器不存在").model_dump()), 404

    new_generation_job = GenerationJob(
        project_id=project_id,
        generator_id=create_generation_job_request.generator_id,
        total_count=create_generation_job_request.count,
        status='Waiting',
        generated_count=0
    )
    db.session.add(new_generation_job)
    db.session.commit()

    return jsonify(PostResponse(status=Status.success, message="生成任务已创建"))


@app.route('/projects/<int:project_id>/generation_job/<int:job_id>', methods=['POST'])
def generation_job_action(project_id, job_id):
    try:
        request_data = GenerationJobActionRequest(**request.json)
    except ValidationError as e:
        return jsonify({"error": str(e)}), 400

    generation_job = GenerationJob.query.filter_by(project_id=project_id, id=job_id).first()

    if not generation_job:
        return jsonify({"error": "Generation job not found"}), 404

    # 根据请求类型进行相应的操作
    if request_data.type == 'start':
        generation_job.status = 'Running'
        task = generate_dialogs.delay(job_id)
        generation_job.task_id = task.id
    elif request_data.type == 'stop':
        generation_job.status = 'Stopped'
        celery_app.control.revoke(generation_job.task_id)
    elif request_data.type == 'retry':
        generation_job.status = 'Running'
        task = generate_dialogs.delay(job_id)
        generation_job.task_id = task.id

    db.session.commit()
    return jsonify(PostResponse(status=Status.success, message='Correct').model_dump_json())


def get_job_progress(job: GenerationJob):
    return float(job.generated_count) / float(job.total_count) if job.total_count > 0 else 0


@app.route('/projects/<int:project_id>/generation_job', methods=['GET'])
def get_all_generation_jobs(project_id):
    try:
        job_filter = request.args.get('filter')
    except Exception as e:
        return jsonify({"status": str(e), "message": "filter parameter not found"}), 400

    if job_filter == 'all':
        generation_jobs = GenerationJob.query.filter_by(project_id=project_id).all()
    elif job_filter == 'unfinished':
        generation_jobs = GenerationJob.query \
            .filter_by(project_id=project_id) \
            .filter(GenerationJob.status.in_(['Running', 'Waiting'])).all()
    else:
        return jsonify({'status': 'error', 'message': 'unknown filter'}), 400

    response_data = []
    for job in generation_jobs:
        response_data.append(GenerationJobResponse(
            id=job.id,
            name=job.name,
            created_at=job.created_at,
            status=job.status,
            progress=get_job_progress(job),
        ))

    return jsonify(GenerationJobListResponse(jobs=response_data).model_dump())


@app.route('/projects/<int:project_id>/generation_job/<int:job_id>', methods=['GET'])
def get_generation_job_detail(project_id, job_id):
    generation_job = GenerationJob.query.filter_by(project_id=project_id, id=job_id).first()
    generator = Generator.query.get(generation_job.generator_id)
    if not generation_job:
        return jsonify({"error": "Generation job not found"}), 404

    duration = generation_job.duration
    duration_timedelta = timedelta(hours=duration.hour, minutes=duration.minute, seconds=duration.second)
    duration_seconds = int(duration_timedelta.total_seconds())

    job_detail_response = JobDetailResponse(
        progress=get_job_progress(generation_job),
        token=generation_job.tokens,
        duration=duration_seconds,
        generator=generator.content,
        config={
            "model": generation_job.model_name,
            "temperature": generation_job.temperature,
            "length": generation_job.tokens
        },
    )

    return jsonify(job_detail_response.model_dump())
