use std::{collections::HashMap, thread::sleep, time::Duration};

use bevy::{app::ScheduleRunnerPlugin, prelude::*};
use bevy_quinnet::{
    client::{
        QuinnetClient, QuinnetClientPlugin,
        certificate::CertificateVerificationMode,
        client_connected,
        connection::{ClientEndpointConfiguration, ConnectionEvent, ConnectionFailedEvent},
    },
    shared::{ClientId, channels::ChannelsConfiguration},
};
use godot::prelude::*;
use godot_bevy::prelude::*;
use rand::{Rng, distributions::Alphanumeric};
use tokio::sync::mpsc;

use protocol::{ClientMessage, ServerMessage};

use crate::chat::{Chat, ChatInput, ChatNode};

mod chat;
mod player;
mod protocol;

use player::SpawnPlayerEvent;

#[derive(Resource, Debug, Clone, Default)]
struct Users {
    self_id: ClientId,
    names: HashMap<ClientId, String>,
}

#[derive(Resource, Deref, DerefMut)]
pub struct ChatReceiver(mpsc::Receiver<String>);

#[derive(Event)]
pub struct ChatMessage {
    pub username: String,
    pub message: String,
}

#[bevy_app]
fn build_app(app: &mut App) {
    app.add_plugins(GodotDefaultPlugins);

    app.add_plugins((
        ScheduleRunnerPlugin::default(),
        QuinnetClientPlugin::default(),
        player::PlayerPlugin,
    ))
    .insert_resource(Users::default())
    .add_systems(
        Startup,
        (hello_world, start_chat_listener, start_connection),
    )
    .add_systems(
        Update,
        (
            handle_client_events,
            (handle_terminal_messages, handle_server_messages).run_if(client_connected),
            chat::read_chat_messages,
            handle_chat_sync,
        ),
    )
    .add_systems(PostUpdate, on_app_exit);

    app.add_event::<ChatMessage>();
}

fn hello_world() {
    godot::prelude::godot_print!("Hello from godot-bevy!");
}

fn start_connection(mut client: ResMut<QuinnetClient>) {
    godot_print!("Starting connection");
    client
        .open_connection(
            ClientEndpointConfiguration::from_strings("[::1]:6000", "[::]:0").unwrap(),
            CertificateVerificationMode::SkipVerification,
            ChannelsConfiguration::default(),
        )
        .unwrap();
}

fn start_chat_listener(mut commands: Commands) {
    let (from_chat_sender, from_chat_receiver) = mpsc::channel::<String>(100);

    // get ChatInputNode
    commands.queue(move |world: &mut World| {
        let mut chat_input_node = world.query::<&mut ChatInput>();
        for mut chat_input_node in chat_input_node.iter_mut(world) {
            chat_input_node.sender = Some(from_chat_sender.clone());
        }
    });

    commands.insert_resource(ChatReceiver(from_chat_receiver));
}

#[main_thread_system]
fn handle_chat_sync(
    mut query: Query<(Entity, &mut GodotNodeHandle, &mut Chat), With<RichTextLabelMarker>>,
    mut _events: EventReader<ChatMessage>,
) {
    for (_, mut handle, chat) in query.iter_mut() {
        let mut rich_text_label = handle.get::<ChatNode>();
        rich_text_label.set_text(&chat.messages.join("\n"));
    }
    _events.clear();
}

fn handle_terminal_messages(
    mut terminal_messages: ResMut<ChatReceiver>,
    mut app_exit_events: EventWriter<AppExit>,
    mut client: ResMut<QuinnetClient>,
) {
    while let Ok(message) = terminal_messages.try_recv() {
        godot_print!("{}", message);
        if message == "quit" {
            app_exit_events.write(AppExit::Success);
        } else {
            client
                .connection_mut()
                .try_send_message(ClientMessage::ChatMessage { message: message });
        }
    }
}

fn handle_client_events(
    mut connection_events: EventReader<ConnectionEvent>,
    mut connection_failed_events: EventReader<ConnectionFailedEvent>,
    mut client: ResMut<QuinnetClient>,
) {
    if !connection_events.is_empty() {
        // We are connected
        let username: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(7)
            .map(char::from)
            .collect();

        godot::prelude::godot_print!("--- Joining with name: {}", username);
        godot::prelude::godot_print!("--- Type 'quit' to disconnect");

        client
            .connection_mut()
            .send_message(ClientMessage::Join { name: username })
            .unwrap();

        connection_events.clear();
    }
    for ev in connection_failed_events.read() {
        godot::prelude::godot_print!(
            "Failed to connect: {:?}, make sure the chat-server is running.",
            ev.err
        );
    }
}

fn handle_server_messages(
    mut users: ResMut<Users>,
    mut client: ResMut<QuinnetClient>,
    mut commands: Commands,
) {
    while let Some((_, message)) = client
        .connection_mut()
        .try_receive_message::<ServerMessage>()
    {
        match message {
            ServerMessage::ClientConnected {
                client_id,
                username,
            } => {
                info!("{} joined", username);
                users.names.insert(client_id, username.clone());

                // Only spawn players for other clients (not ourselves)
                // Our own player will be spawned in the InitClient handler
                if client_id != users.self_id {
                    godot_print!("Sending spawn event for remote client ID: {:?}", client_id);
                    commands.send_event(SpawnPlayerEvent {
                        client_id,
                        position: None, // Use default position in scene
                    });
                }

                commands.queue(move |world: &mut World| {
                    let mut chat_node = world.query::<&mut Chat>();
                    for mut chat_node in chat_node.iter_mut(world) {
                        chat_node.messages.push(format!("{} joined", username));
                    }
                    // Send event to sync chat
                    world.send_event(ChatMessage {
                        username: username.clone(),
                        message: format!("{} joined", username),
                    });
                });
            }
            ServerMessage::ClientDisconnected { client_id } => {
                if let Some(username) = users.names.remove(&client_id) {
                    godot::prelude::godot_print!("{} left", username.clone());
                    commands.queue(move |world: &mut World| {
                        // Update chat
                        let mut chat_node = world.query::<&mut Chat>();
                        for mut chat_node in chat_node.iter_mut(world) {
                            chat_node.messages.push(format!("{} left", username));
                        }
                        // Send event to sync chat
                        world.send_event(ChatMessage {
                            username: username.clone(),
                            message: format!("{} left", username),
                        });

                        // Find and destroy the player entity for this client
                        let mut to_destroy = Vec::new();

                        // First, find all player node handles associated with this client ID
                        let mut query =
                            world.query::<(&player::Player, &mut GodotNodeHandle, Entity)>();
                        for (player, mut handle, entity) in query.iter_mut(world) {
                            if player.0 == client_id {
                                godot_print!(
                                    "Destroying player entity for disconnected client: {}",
                                    client_id
                                );

                                // Free the Godot node
                                if let Some(mut player_node) =
                                    handle.try_get::<player::PlayerNode>()
                                {
                                    player_node.queue_free();
                                    godot_print!("Queued Godot player node for freeing");
                                }

                                // Mark this entity for destruction
                                to_destroy.push(entity);
                            }
                        }

                        // Now destroy all marked entities
                        for entity in to_destroy {
                            world.despawn(entity);
                        }
                    });
                } else {
                    warn!("ClientDisconnected for an unknown client_id: {}", client_id);
                }
            }
            ServerMessage::ChatMessage { client_id, message } => {
                if let Some(username) = users.names.get(&client_id) {
                    let username = username.clone(); // Clone here to own the data
                    if client_id != users.self_id {
                        godot::prelude::godot_print!("{}: {}", username, message);
                    }
                    commands.queue(move |world: &mut World| {
                        let mut chat_node = world.query::<&mut Chat>();
                        for mut chat_node in chat_node.iter_mut(world) {
                            chat_node
                                .messages
                                .push(format!("{}: {}", username, message));
                        }
                        // Send event to sync chat
                        world.send_event(ChatMessage { username, message });
                    });
                } else {
                    warn!("Chat message from an unknown client_id: {}", client_id)
                }
            }
            ServerMessage::InitClient {
                client_id,
                usernames,
            } => {
                godot_print!("Setting self_id to: {:?}", client_id);
                users.self_id = client_id;
                users.names = usernames;

                // Spawn player for self after we've received our own client_id
                godot_print!(
                    "Sending spawn event for local player with client ID: {:?}",
                    client_id
                );
                commands.send_event(SpawnPlayerEvent {
                    client_id,
                    position: None, // Use default position in scene
                });

                // Spawn all other existing players
                for &other_client_id in users.names.keys() {
                    // Don't spawn our own player twice
                    if other_client_id != client_id {
                        godot_print!(
                            "Spawning existing player with client ID: {:?}",
                            other_client_id
                        );
                        commands.send_event(SpawnPlayerEvent {
                            client_id: other_client_id,
                            position: None, // Use default position in scene
                        });
                    }
                }
            }
            ServerMessage::PlayerUpdate {
                client_id,
                x,
                y,
                horizontal,
                vertical,
            } => {
                let player_id = users.self_id.clone();
                commands.queue(move |world: &mut World| {
                    // query the player node by client_id
                    let mut player_query = world.query::<&mut GodotNodeHandle>();
                    for mut handle in player_query.iter_mut(world) {
                        let player_node = handle.try_get::<player::PlayerNode>();
                        if player_node.is_none() {
                            continue;
                        }
                        let mut player_node = player_node.unwrap();

                        // Only update remote players - never override local player position
                        if player_node.bind().client_id == client_id as u32
                            && client_id != player_id
                        {
                            // First, check if position is significantly different (to prevent small jitters)
                            let current_pos = player_node.get_position();
                            let distance =
                                ((current_pos.x - x).powi(2) + (current_pos.y - y).powi(2)).sqrt();
                            // Only update if there's a significant change (more than 2 pixels)
                            if distance > 2.0 {
                                player_node.set_position(Vector2::new(x, y));
                            }
                        }
                    }
                    world.send_event(player::PlayerInputEvent {
                        client_id,
                        horizontal,
                        vertical,
                    });
                });
            }
        }
    }
}

pub fn on_app_exit(app_exit_events: EventReader<AppExit>, mut client: ResMut<QuinnetClient>) {
    if !app_exit_events.is_empty() {
        client
            .connection_mut()
            .send_message(ClientMessage::Disconnect {})
            .unwrap();
        // TODO Clean: event to let the async client send his last messages.
        sleep(Duration::from_secs_f32(0.1));
    }
}
