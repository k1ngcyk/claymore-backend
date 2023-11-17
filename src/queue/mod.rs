use lapin::{
    message::DeliveryResult,
    options::{
        BasicAckOptions, BasicConsumeOptions, BasicNackOptions, BasicPublishOptions,
        BasicQosOptions, QueueDeclareOptions,
    },
    publisher_confirm::Confirmation,
    types::FieldTable,
    BasicProperties, Channel, Connection, ConnectionProperties,
};
use serde_json::Value;
use sqlx::PgPool;

mod executor;

pub async fn make_channel(url: &String) -> Channel {
    let uri = url;
    let options = ConnectionProperties::default()
        // Use tokio executor and reactor.
        // At the moment the reactor is only available for unix.
        .with_executor(tokio_executor_trait::Tokio::current())
        .with_reactor(tokio_reactor_trait::Tokio);
    let connection = Connection::connect(uri, options).await.unwrap();
    let channel = connection.create_channel().await.unwrap();
    channel
}

pub async fn start_consumer(db: PgPool, mq: Channel) {
    let db = db.clone();
    let db2 = db.clone();
    let db_eval = db.clone();
    let channel = mq;
    channel
        .basic_qos(1, BasicQosOptions::default())
        .await
        .unwrap();
    let _queue = channel
        .queue_declare(
            "claymore_job_queue",
            QueueDeclareOptions::default(),
            FieldTable::default(),
        )
        .await
        .unwrap();
    let _queue_v2 = channel
        .queue_declare(
            "claymore_v2_queue",
            QueueDeclareOptions::default(),
            FieldTable::default(),
        )
        .await
        .unwrap();
    let _queue_v2_evaluate = channel
        .queue_declare(
            "claymore_v2_evaluate_queue",
            QueueDeclareOptions::default(),
            FieldTable::default(),
        )
        .await
        .unwrap();
    let consumer = channel
        .basic_consume(
            "claymore_job_queue",
            "tag_job_consumer",
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await
        .unwrap();
    let consumer_v2 = channel
        .basic_consume(
            "claymore_v2_queue",
            "tag_v2_consumer",
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await
        .unwrap();
    let consumer_v2_evaluate = channel
        .basic_consume(
            "claymore_v2_evaluate_queue",
            "tag_v2_evaluate_consumer",
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await
        .unwrap();

    consumer.set_delegate(move |delivery: DeliveryResult| {
        let db = db.clone();
        async move {
            let delivery = match delivery {
                // Carries the delivery alongside its channel
                Ok(Some(delivery)) => delivery,
                // The consumer got canceled
                Ok(None) => return,
                // Carries the error and is always followed by Ok(None)
                Err(error) => {
                    log::error!("Consumer error: {}", error);
                    return;
                }
            };

            let result = match executor::execute_job(db, &delivery).await {
                Ok(result) => result,
                Err(error) => {
                    log::error!("Execute job error: {}", error);
                    delivery
                        .ack(BasicAckOptions::default())
                        .await
                        .expect("Failed to ack message");
                    return;
                }
            };

            match result {
                executor::ExecuteResult::Overflow => {
                    delivery
                        .ack(BasicAckOptions::default())
                        .await
                        .expect("Failed to ack message");
                }
                executor::ExecuteResult::Success => {
                    delivery
                        .nack(BasicNackOptions {
                            multiple: false,
                            requeue: true,
                        })
                        .await
                        .expect("Failed to requeue message");
                }
            }
        }
    });

    consumer_v2.set_delegate(move |delivery: DeliveryResult| {
        let db = db2.clone();
        async move {
            let delivery = match delivery {
                // Carries the delivery alongside its channel
                Ok(Some(delivery)) => delivery,
                // The consumer got canceled
                Ok(None) => return,
                // Carries the error and is always followed by Ok(None)
                Err(error) => {
                    log::error!("Consumer error: {}", error);
                    return;
                }
            };

            let result = match executor::execute_job_v2(db, &delivery).await {
                Ok(result) => result,
                Err(error) => {
                    log::error!("Execute job error: {}", error);
                    delivery
                        .ack(BasicAckOptions::default())
                        .await
                        .expect("Failed to ack message");
                    return;
                }
            };

            match result {
                executor::ExecuteResultV2::Success => {
                    delivery
                        .ack(BasicAckOptions::default())
                        .await
                        .expect("Failed to ack message");
                }
                executor::ExecuteResultV2::Failed => {
                    delivery
                        .nack(BasicNackOptions {
                            multiple: false,
                            requeue: true,
                        })
                        .await
                        .expect("Failed to requeue message");
                }
            }
        }
    });

    consumer_v2_evaluate.set_delegate(move |delivery: DeliveryResult| {
        let db = db_eval.clone();
        async move {
            let delivery = match delivery {
                // Carries the delivery alongside its channel
                Ok(Some(delivery)) => delivery,
                // The consumer got canceled
                Ok(None) => return,
                // Carries the error and is always followed by Ok(None)
                Err(error) => {
                    log::error!("Consumer error: {}", error);
                    return;
                }
            };

            let result = match executor::execute_job_v2_evaluate(db, &delivery).await {
                Ok(result) => result,
                Err(error) => {
                    log::error!("Execute job error: {}", error);
                    delivery
                        .ack(BasicAckOptions::default())
                        .await
                        .expect("Failed to ack message");
                    return;
                }
            };

            match result {
                executor::ExecuteResultV2::Success => {
                    delivery
                        .ack(BasicAckOptions::default())
                        .await
                        .expect("Failed to ack message");
                }
                executor::ExecuteResultV2::Failed => {
                    delivery
                        .nack(BasicNackOptions {
                            multiple: false,
                            requeue: true,
                        })
                        .await
                        .expect("Failed to requeue message");
                }
            }
        }
    });
}

pub async fn publish_message(channel: &Channel, message: Value) -> Confirmation {
    let result = channel
        .basic_publish(
            "",
            "claymore_job_queue",
            BasicPublishOptions::default(),
            message.to_string().as_bytes(),
            BasicProperties::default(),
        )
        .await
        .unwrap()
        .await
        .unwrap();
    result
}

pub async fn publish_message_v2(channel: &Channel, message: Value) -> Confirmation {
    let result = channel
        .basic_publish(
            "",
            "claymore_v2_queue",
            BasicPublishOptions::default(),
            message.to_string().as_bytes(),
            BasicProperties::default(),
        )
        .await
        .unwrap()
        .await
        .unwrap();
    result
}

pub async fn publish_message_v2_evaluate(channel: &Channel, message: Value) -> Confirmation {
    let result = channel
        .basic_publish(
            "",
            "claymore_v2_evaluate_queue",
            BasicPublishOptions::default(),
            message.to_string().as_bytes(),
            BasicProperties::default(),
        )
        .await
        .unwrap()
        .await
        .unwrap();
    result
}
