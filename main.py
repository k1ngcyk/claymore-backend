from setup import app, db, celery_app
from endpoints import *

if __name__ == '__main__':
    celery_app.start()
    app.run(debug=True)

