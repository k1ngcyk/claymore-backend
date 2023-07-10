import os
import time
from datetime import datetime

import openai

from setup import db, celery_app

from models.orms import Dialog, GenerationJob
from openai.error import (
    APIError,
    APIConnectionError,
    ServiceUnavailableError,
    InvalidRequestError,
    AuthenticationError,
    PermissionError,
    RateLimitError)


def generate(prompt: str, model='gpt-3.5-turbo'):
    answer = openai.ChatCompletion.create(
        model=model,
        messages=[

            {"role": "user", "content": prompt},
        ],
        temperature=0,
    )['choices'][0]['message']['content']
    return answer


@celery_app.task(bind=True, name='generate_dialogs')
def generate_dialogs(self, job_id):
    job = db.query(GenerationJob).get(job_id)

    if not job:
        return

    job.status = 'Running'
    db.commit()

    retries = 3
    success_count = 0

    for _ in range(job.total_count):
        # Make API call
        for attempt in range(retries):
            try:
                response = generate('prompt')  # todo

                # Save successful response to Dialog
                dialog = Dialog(project_id=job.project_id,
                                content=response['content'],
                                source_type='Generator',
                                source_id=job.generator_id,
                                status='Candidate',
                                created_at=datetime.now(),
                                attrs=response['attrs'])
                db.add(dialog)
                success_count += 1
                db.commit()
                break

            except (APIError, TimeoutError, APIConnectionError, ServiceUnavailableError) as e:
                if attempt < retries - 1:
                    time.sleep(2 ** attempt)  # Exponential backoff
                else:
                    job.status = 'Stopped'
                    db.commit()
                    return

            except (InvalidRequestError, AuthenticationError, PermissionError, RateLimitError) as e:
                job.status = 'Error'
                db.commit()
                return

    job.status = 'Finished'
    job.generated_count = success_count
    db.commit()
