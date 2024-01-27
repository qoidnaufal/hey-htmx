use axum::extract::ws::{Message, WebSocket};
use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::sync::mpsc;

use crate::{RegisteredUsers, User};

#[derive(Debug, Deserialize)]
struct Msg {
    message: String,
}

pub async fn ws_connection(
    ws: WebSocket,
    email: String,
    registered_users: RegisteredUsers,
    mut user: User,
) {
    println!("INFO: New client: {} is connected", email);

    let (mut sender, mut receiver) = ws.split();

    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();

    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            match sender.send(msg).await {
                Ok(_) => (),
                Err(err) => eprintln!("Unable to send ws message: {}", err),
            }
        }
        sender.close().await.unwrap();
    });

    user.sender = Some(tx);

    registered_users
        .write()
        .unwrap()
        .insert(email.clone(), user);

    let user_name = registered_users
        .read()
        .unwrap()
        .get(&email)
        .unwrap()
        .user_name
        .clone();

    while let Some(Ok(message)) = receiver.next().await {
        let message = match message {
            Message::Text(msg) => {
                println!("INFO: Received message: {}", msg);

                let deserialized_msg = serde_json::from_str::<Msg>(&msg)
                    .map_err(|err| eprintln!("Unable to deserialize the message: {}", err))
                    .unwrap();

                let message_template = format!(
                    "<div hx-swap-oob='beforeend:#log'><p>{}: {}</p></div>",
                    user_name.clone(),
                    deserialized_msg.message.clone()
                );

                let msg_to_send = serde_json::to_string(&message_template)
                    .map_err(|err| eprintln!("Unable to serialize the message: {}", err))
                    .unwrap();
                Message::Text(msg_to_send)
            }
            _ => message,
        };

        broadcast_msg(message, &registered_users).await;
    }

    registered_users.write().unwrap().remove(&email);
}

pub async fn broadcast_msg(msg: Message, registered_users: &RegisteredUsers) {
    if let Message::Text(message) = msg {
        for (email, user) in registered_users.read().unwrap().iter() {
            if let Some(tx) = user.sender.clone() {
                match tx.send(Message::Text(message.clone())) {
                    Ok(_) => println!("INFO: Sent message: {}", message.clone()),
                    Err(err) => eprintln!("Unable to send message from: {}, : {}", email, err),
                }
            }
        }
    }
}

pub async fn register_user(
    uuid: String,
    user_name: String,
    email: String,
    password: String,
    registered_users: RegisteredUsers,
) -> Result<(), String> {
    if registered_users.write().unwrap().get(&email).is_none() {
        match registered_users.write().unwrap().insert(
            email.clone(),
            User {
                uuid,
                user_name,
                email,
                password,
                sender: None,
            },
        ) {
            None => Ok(()),
            Some(n) => Err(format!(
                "User with email \"{}\" is already registered",
                n.email
            )),
        }
    } else {
        Err(format!(
            "User with email \"{}\" is already registered",
            email
        ))
    }
}