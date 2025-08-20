use bevy::prelude::*;
use bevy_quinnet::shared::ClientId;
use godot::{
    classes::{AnimatedSprite2D, CharacterBody2D, Input, ResourceLoader},
    prelude::*,
};
use godot_bevy::prelude::*;

use crate::Users;

const PLAYER_SPEED: f32 = 150.0;
const INPUT_DEADZONE: f32 = 0.2;

#[derive(Component, Default, Clone, Copy)]
pub struct Player(pub ClientId);

#[derive(Component, Default, Clone, Copy)]
pub struct PlayerFacing(pub FacingDir);

// Persist last known input for smooth motion/animation across frames
#[derive(Component, Default, Clone, Copy)]
pub struct PlayerInputState {
    pub horizontal: f32,
    pub vertical: f32,
}

// Track last played animation to avoid restarting the same animation every frame
#[derive(Component, Default, Clone)]
pub struct PlayerAnimState {
    pub current: String,
}
#[derive(Event)]
pub struct SpawnPlayerEvent {
    pub client_id: ClientId,
    pub position: Option<Vector2>,
}

#[derive(Event, Default, Clone)]
pub struct PlayerInputEvent {
    pub client_id: ClientId,
    pub horizontal: f32,
    pub vertical: f32,
}

// Player facing direction (cardinal only)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FacingDir {
    Up,
    Down,
    Left,
    Right,
}

impl Default for FacingDir {
    fn default() -> Self {
        FacingDir::Down
    }
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum PlayerSystemSet {
    /// Input detection (can run in parallel with other input systems)
    InputDetection,
    /// Physics and movement (runs after input detection)
    Movement,
    /// Animation updates (runs after movement)
    Animation,
    /// Player spawning
    Spawning,
}

#[derive(GodotClass)]
#[class(base=CharacterBody2D, init)]
pub struct PlayerNode {
    base: Base<CharacterBody2D>,
    #[var]
    pub client_id: u32,
}

#[derive(Resource)]
pub struct PlayerSceneResource {
    pub scene_path: String,
}

impl Default for PlayerSceneResource {
    fn default() -> Self {
        Self {
            scene_path: "res://player.tscn".to_string(),
        }
    }
}

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlayerSceneResource>()
            .add_event::<PlayerInputEvent>()
            .add_event::<SpawnPlayerEvent>()
            .add_systems(
                PhysicsUpdate,
                (
                    player_input_system.in_set(PlayerSystemSet::InputDetection),
                    player_movement_system.in_set(PlayerSystemSet::Movement),
                    player_animation_system.in_set(PlayerSystemSet::Animation),
                )
                    .chain(),
            )
            .add_systems(
                Update,
                player_spawner_system.in_set(PlayerSystemSet::Spawning),
            );
    }
}

#[main_thread_system]
fn player_spawner_system(
    mut commands: Commands,
    mut spawn_events: EventReader<SpawnPlayerEvent>,
    scene_resource: Res<PlayerSceneResource>,
) {
    for event in spawn_events.read() {
        godot_print!("Spawning player for client: {:?}", event.client_id);

        // Load the player scene
        let mut resource_loader = ResourceLoader::singleton();
        let packed_scene = resource_loader
            .load(&scene_resource.scene_path.clone())
            .expect("Failed to load player scene");

        // Cast to PackedScene
        let packed_scene = packed_scene.cast::<PackedScene>();

        // Instantiate the scene
        let instance = packed_scene
            .instantiate()
            .expect("Failed to instantiate player scene");

        // Get the root node as CharacterBody2D
        let character = instance.try_cast::<PlayerNode>();
        if let Ok(mut character) = character {
            // Set initial position if provided
            if let Some(position) = event.position {
                character.set_position(position);
            }

            // Set the client_id field directly on the PlayerNode
            let raw_id = event.client_id;
            godot_print!("Setting player node client_id field to: {}", raw_id);
            character.bind_mut().client_id = raw_id.try_into().unwrap();

            // Create the Bevy entity FIRST (before adding to scene tree)
            let entity = commands.spawn((
                GodotNodeHandle::new(character.clone()),
                Player(event.client_id),
                PlayerFacing::default(),
                PlayerInputState::default(),
                PlayerAnimState::default(),
            ));

            godot_print!(
                "Created entity ID: {:?} with client ID: {:?}",
                entity.id(),
                event.client_id
            );

            // Now add to scene tree AFTER creating the entity
            let mut root = godot::classes::Engine::singleton()
                .get_main_loop()
                .and_then(|ml| ml.try_cast::<SceneTree>().ok())
                .and_then(|tree| tree.get_current_scene())
                .expect("Failed to get current scene");

            // First add to the scene tree
            root.add_child(&character);

            // Try multiple ways to ensure position is set
            // Set position with a random X coordinate between 200 and 600
            let random_x = rand::random::<f32>() * 400.0 + 200.0;
            character.set_position(Vector2::new(random_x, 100.0));
            character.set_global_position(Vector2::new(random_x, 100.0));

            character.set_velocity(Vector2::ZERO);

            godot_print!(
                "Player spawned and added to scene with client ID: {:?}",
                event.client_id
            );
        } else {
            godot_print!("Failed to cast player scene to CharacterBody2D");
        }
    }
}

#[main_thread_system]
fn player_input_system(
    mut query: Query<(&Player, &mut GodotNodeHandle)>,
    mut input_events: EventWriter<PlayerInputEvent>,
    mut client: ResMut<bevy_quinnet::client::QuinnetClient>,
    users: Res<Users>,
) {
    for (player, mut handle) in query.iter_mut() {
        let player_node = handle.try_get::<PlayerNode>();
        if player_node.is_none() {
            continue;
        }
        let player_node = player_node.unwrap();

        let node_client_id = player_node.bind().client_id;
        let component_client_id = player.0;

        // Check both the component's ClientId and the node's client_id field
        if component_client_id == users.self_id || node_client_id == users.self_id as u32 {
            let input = Input::singleton();
            let mut horizontal = input.get_axis("ui_left", "ui_right");
            let mut vertical = input.get_axis("ui_up", "ui_down");
            if horizontal.abs() < INPUT_DEADZONE {
                horizontal = 0.0;
            }
            if vertical.abs() < INPUT_DEADZONE {
                vertical = 0.0;
            }

            let player_node = handle.get::<CharacterBody2D>();

            input_events.write(PlayerInputEvent {
                client_id: users.self_id,
                horizontal,
                vertical,
            });

            client.connection_mut().try_send_message(
                crate::protocol::ClientMessage::PlayerUpdate {
                    x: player_node.get_position().x,
                    y: player_node.get_position().y,
                    horizontal,
                    vertical,
                },
            );

            // We found our player, no need to check others
            break;
        }
    } // End of for loop
}

#[main_thread_system]
fn player_movement_system(
    mut input_events: EventReader<PlayerInputEvent>,
    mut query: Query<(
        &Player,
        &mut GodotNodeHandle,
        &mut PlayerFacing,
        &mut PlayerInputState,
    )>,
    _physics_delta: Res<PhysicsDelta>,
) {
    // Collect input events by client_id for faster lookup
    let mut input_by_client = std::collections::HashMap::new();
    for input_event in input_events.read() {
        input_by_client.insert(input_event.client_id, input_event.clone());
    }

    // Process all players
    for (player, mut handle, mut facing, mut input_state) in query.iter_mut() {
        let client_id = player.0;
        let player_node = handle.try_get::<PlayerNode>();
        if player_node.is_none() {
            continue;
        }
        let mut player_node = player_node.unwrap();

        // Start with zero velocity
        let mut velocity = Vector2::ZERO;

        // Determine effective input for this player, persist when new input arrives
        let mut h = input_state.horizontal;
        let mut v = input_state.vertical;
        if let Some(input) = input_by_client.get(&client_id) {
            h = input.horizontal;
            v = input.vertical;
            // Deadzone filtering
            if h.abs() < INPUT_DEADZONE {
                h = 0.0;
            }
            if v.abs() < INPUT_DEADZONE {
                v = 0.0;
            }
            // Persist
            input_state.horizontal = h;
            input_state.vertical = v;
        }

        // Compute velocity and facing from persisted input
        if h != 0.0 || v != 0.0 {
            velocity.x = h * PLAYER_SPEED;
            velocity.y = v * PLAYER_SPEED;
            // Update facing to the primary cardinal direction
            let ax = h.abs();
            let ay = v.abs();
            facing.0 = if ax >= ay {
                if h >= 0.0 {
                    FacingDir::Right
                } else {
                    FacingDir::Left
                }
            } else if v >= 0.0 {
                FacingDir::Down
            } else {
                FacingDir::Up
            };
            velocity = velocity.normalized() * PLAYER_SPEED;
        }

        // Apply to Godot node
        player_node.set_velocity(velocity);
        player_node.move_and_slide();
    }
}

#[main_thread_system]
fn player_animation_system(
    mut query: Query<(
        &Player,
        &mut GodotNodeHandle,
        &PlayerFacing,
        &PlayerInputState,
        &mut PlayerAnimState,
    )>,
) {
    for (_player, mut handle, facing, input_state, mut anim_state) in query.iter_mut() {
        let player_node = handle.try_get::<PlayerNode>();
        if player_node.is_none() {
            continue;
        }
        let player_node = player_node.unwrap();
        let is_moving = input_state.horizontal.abs() >= INPUT_DEADZONE
            || input_state.vertical.abs() >= INPUT_DEADZONE;

        // Determine facing direction (persisted on component)
        let dir_str = match facing.0 {
            FacingDir::Up => "up",
            FacingDir::Down => "down",
            FacingDir::Left => "left",
            FacingDir::Right => "right",
        };

        let anim_name = if is_moving {
            format!("run_{}", dir_str)
        } else {
            format!("idle_{}", dir_str)
        };

        // Only switch animation if it changed to prevent restarts/glitches
        if anim_state.current != anim_name {
            let mut sprite = player_node.get_node_as::<AnimatedSprite2D>("AnimatedSprite2D");
            sprite.play_ex().name(&anim_name).done();
            anim_state.current = anim_name;
        }
    }
}
