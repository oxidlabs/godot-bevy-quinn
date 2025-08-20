use std::collections::HashMap;

use bevy_quinnet::shared::ClientId;
use serde::{Deserialize, Serialize};

// Messages from clients
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientMessage {
    Join {
        name: String,
    },
    Disconnect {},
    ChatMessage {
        message: String,
    },
    PlayerUpdate {
        x: f32,
        y: f32,
        horizontal: f32,
        vertical: f32,
    },
}

// Messages from the server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerMessage {
    ClientConnected {
        client_id: ClientId,
        username: String,
    },
    ClientDisconnected {
        client_id: ClientId,
    },
    ChatMessage {
        client_id: ClientId,
        message: String,
    },
    InitClient {
        client_id: ClientId,
        usernames: HashMap<ClientId, String>,
    },
    PlayerUpdate {
        client_id: ClientId,
        x: f32,
        y: f32,
        horizontal: f32,
        vertical: f32,
    },
}
