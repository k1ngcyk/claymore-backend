import os
import time
from datetime import datetime, timedelta

import openai

from setup import db, celery_app

from models.orms import Dialog, GenerationJob, Generator
from openai.error import (
    APIError,
    APIConnectionError,
    ServiceUnavailableError,
    InvalidRequestError,
    AuthenticationError,
    PermissionError,
    RateLimitError)


def generate(prompt: str, model='gpt-3.5-turbo', temperature=0):
    answer = openai.ChatCompletion.create(
        model=model,
        messages=[
            # {"role": "system", "content": "You're ChatGPT plus."},
            {"role": "user", "content": prompt},
        ],
        temperature=temperature,
    )['choices'][0]['message']['content']
    return answer


def get_new_duration(time_start: float,
                     time_end: float,
                     old_duration: datetime.time) -> datetime.time:
    delta = timedelta(seconds=time_end - time_start)
    current_duration = datetime.combine(datetime.min, old_duration)

    # 应用 timedelta
    new_duration_datetime = current_duration + delta

    # 提取新的时间部分
    new_duration = new_duration_datetime.time()
    return new_duration


@celery_app.task(bind=True, name='generate_dialogs')
def generate_dialogs(job_id):
    job = db.query(GenerationJob).get(job_id)
    generator = db.query(Generator).filter_by(id=job.generator_id).first()

    if not job or not generator:
        return

    job.status = 'Running'
    db.commit()

    retries = 3

    time_start = time.time()

    model_name = job.model_name
    temperature = job.temperature

    items_left = job.total_count - job.generated_count
    if items_left == 0:
        return

    for _ in range(items_left):
        # Make API call
        for attempt in range(retries):
            try:
                if len(generator.content) == 1:
                    response = generate(generator.content[0], model_name, temperature)
                else:
                    start_prompt = generator.content[0]
                    response = generate(start_prompt, model_name, temperature)
                    for prompt in generator.content[1:]:
                        response = generate(prompt.replace('^^', response), model_name, temperature)

                # Save successful response to Dialog
                dialog = Dialog(project_id=job.project_id,
                                content=response,
                                source_type='Generator',
                                source_id=job.generator_id,
                                status='Candidate',
                                created_at=datetime.now(),
                                attrs={})
                db.add(dialog)
                job.generated_count += 1
                db.commit()
                break

            except (APIError, TimeoutError, APIConnectionError, ServiceUnavailableError) as e:
                if attempt < retries - 1:
                    time.sleep(2 ** attempt)  # Exponential backoff
                else:
                    job.status = 'Stopped'
                    time_end = time.time()
                    # 更新 duration 字段
                    job.duration = get_new_duration(time_start, time_end, job.duration)

                    db.commit()
                    return

            except (InvalidRequestError, AuthenticationError, PermissionError, RateLimitError) as e:
                job.status = 'Error'
                time_end = time.time()
                job.duration = get_new_duration(time_start, time_end, job.duration)
                db.commit()
                return

    job.status = 'Finished'
    time_end = time.time()
    # 更新 duration 字段
    job.duration = get_new_duration(time_start, time_end, job.duration)
    db.commit()
