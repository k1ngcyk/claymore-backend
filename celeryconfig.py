# celeryconfig.py
from datetime import timedelta

broker_url = 'redis://localhost:6379/0'
result_backend = 'redis://localhost:6379/0'

task_serializer = 'json'
result_serializer = 'json'
accept_content = ['json']

task_annotations = {
    'myapp.tasks.generate_gpt_content': {'rate_limit': '10/m'},
}

task_time_limit = 300
task_soft_time_limit = 240
worker_prefetch_multiplier = 1
task_acks_late = True
task_reject_on_worker_lost = True

beat_schedule = {
    'retry_gpt_content_generation': {
        'task': 'myapp.tasks.retry_gpt_content_generation',
        'schedule': timedelta(minutes=5),
    },
}