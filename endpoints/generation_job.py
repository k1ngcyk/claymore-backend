app = Flask(__name__)

# 创建数据库连接和会话



@app.route('/projects/<int:project_id>/generator', methods=['POST'])
def add_generator(project_id):
    req_data = request.json
    try:
        add_generator_request = AddGeneratorRequest(**req_data)
    except ValueError as e:
        return jsonify(PostResponse(status="error", message=str(e))), 400

    new_generator = Generator(name=add_generator_request.name, content=add_generator_request.content, project_id=project_id)
    db.session.add(new_generator)
    db.session.commit()

    return jsonify(PostResponse(status="success", message="生成器已添加"))

@app.route('/projects/<int:project_id>/generation_job', methods=['POST'])
def create_generation_job(project_id):
    req_data = request.json
    try:
        create_generation_job_request = CreateGenerationJobRequest(**req_data)
    except ValueError as e:
        return jsonify(PostResponse(status="error", message=str(e))), 400

    generator = Generator.query.get(create_generation_job_request.generator_id)

    if not generator:
        return jsonify(PostResponse(status="error", message="生成器不存在")), 404

    new_generation_job = GenerationJob(
        project_id=project_id,
        generator_id=create_generation_job_request.generator_id,
        total_count=create_generation_job_request.count,
        status='Waiting',
        generated_count=0
    )
    db.session.add(new_generation_job)
    db.session.commit()

    return jsonify(PostResponse(status="success", message="生成任务已创建"))

@app.route('/projects/<int:project_id>/generation_job/<int:id>', methods=['POST'])
def generation_job_action(project_id, id):
    try:
        request_data = GenerationJobActionRequest.parse_raw(request.data)
    except ValidationError as e:
        return jsonify({"error": str(e)}), 400

    generation_job = db.query(GenerationJob).filter_by(project_id=project_id, id=id).first()

    if not generation_job:
        return jsonify({"error": "GenerationJob not found"}), 404

    # 根据请求类型进行相应的操作
    if request_data.type == 'start':
        generation_job.status = 'Running'
    elif request_data.type == 'stop':
        generation_job.status = 'Stopped'
    elif request_data.type == 'retry':
        generation_job.status = 'Running'
    elif request_data.type == 'create':
        # 在这里添加创建新任务的逻辑
        pass

    db.commit()
    return jsonify(PostResponse())


@app.route('/projects/<int:project_id>/generation_job', methods=['GET'])
def get_all_generation_jobs(project_id):
    try:
        request_data = GetAllGenerationJobsRequest.parse_raw(request.args.get('filter'))
    except ValidationError as e:
        return jsonify({"error": str(e)}), 400

    if request_data.filter == 'all':
        generation_jobs = db.query(GenerationJob).filter_by(project_id=project_id).all()
    elif request_data.filter == 'unfinished':
        generation_jobs = db.query(GenerationJob).filter_by(project_id=project_id).filter(GenerationJob.status != 'Finished').all()

    response_data = []
    for job in generation_jobs:
        response_data.append({
            "id": job.id,
            "name": job.name,
            "created_at": job.created_at,
            "status": job.status,
            "progress": float(job.generated_count) / float(job.total_count) if job.total_count > 0 else 0
        })

    return jsonify({"jobs": response_data})


@app.route('/projects/<int:project_id>/generation_job/<int:id>', methods=['GET'])
def get_generation_job_detail(project_id, id):
    generation_job = db.query(GenerationJob).filter_by(project_id=project_id, id=id).first()

    if not generation_job:
        return jsonify({"error": "GenerationJob not found"}), 404

    job_detail_response = JobDetailResponse(
        progress=float(generation_job.generated_count) / float(generation_job.total_count) if generation_job.total_count > 0 else 0,
        token=generation_job.tokens,
        duration=generation_job.duration,
        generator=[],  # 这里应该填充实际的生成器数据
        config={
            "model": generation_job.model_name,
            "temperature": generation_job.temperature,
            "length": generation_job.tokens
        },
        feedback=0  # 这里应该填充实际的反馈数据
    )

    return jsonify(job_detail_response.dict())


if __name__ == '__main__':
    app.run()