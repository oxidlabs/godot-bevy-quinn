use bevy::prelude::*;
use godot::{
    classes::{Button, IButton},
    prelude::*,
};
use godot_bevy::prelude::*;
use tokio::sync::mpsc::Sender;

#[derive(Clone, Debug)]
pub enum UiCommand {
    Host { server_path: Option<String> },
    Connect,
}

#[derive(Component, Default)]
pub struct HostButtonComp;

#[derive(Component, Default)]
pub struct JoinButtonComp;

#[derive(GodotClass, BevyBundle)]
#[class(base=Button)]
#[bevy_bundle((HostButtonComp))]
pub struct HostButtonNode {
    base: Base<Button>,
    #[export]
    pub server_path: GString,
    #[bevy_bundle]
    pub sender: Option<Sender<UiCommand>>,
}

#[derive(GodotClass, BevyBundle)]
#[class(base=Button)]
#[bevy_bundle((JoinButtonComp))]
pub struct JoinButtonNode {
    base: Base<Button>,
    #[bevy_bundle]
    pub sender: Option<Sender<UiCommand>>,
}

#[godot_api]
impl IButton for HostButtonNode {
    fn init(base: Base<Button>) -> Self {
        Self {
            base,
            server_path: GString::from("server.exe"),
            sender: None,
        }
    }

    fn pressed(&mut self) {
        if let Some(sender) = &self.sender {
            let path = if self.server_path.is_empty() {
                None
            } else {
                Some(self.server_path.to_string())
            };
            let _ = sender.try_send(UiCommand::Host { server_path: path });
        } else {
            godot_print!("Host button pressed, but sender not set yet");
        }
    }
}

#[godot_api]
impl IButton for JoinButtonNode {
    fn init(base: Base<Button>) -> Self {
        Self { base, sender: None }
    }

    fn pressed(&mut self) {
        if let Some(sender) = &self.sender {
            let _ = sender.try_send(UiCommand::Connect);
        } else {
            godot_print!("Join button pressed, but sender not set yet");
        }
    }
}

#[derive(Resource, Deref, DerefMut)]
pub struct UiReceiver(pub tokio::sync::mpsc::Receiver<UiCommand>);

#[main_thread_system]
pub fn start_ui_listener(mut commands: Commands) {
    let (tx, rx) = tokio::sync::mpsc::channel::<UiCommand>(100);

    // Assign the sender to any Host/Join buttons present in the scene
    commands.queue(move |world: &mut World| {
        let mut query = world.query::<&mut GodotNodeHandle>();
        for mut handle in query.iter_mut(world) {
            if let Some(mut host_btn) = handle.try_get::<HostButtonNode>() {
                host_btn.bind_mut().sender = Some(tx.clone());
            }
            if let Some(mut join_btn) = handle.try_get::<JoinButtonNode>() {
                join_btn.bind_mut().sender = Some(tx.clone());
            }
        }
    });

    commands.insert_resource(UiReceiver(rx));
}

#[main_thread_system]
pub fn handle_ui_commands(
    mut ui_rx: ResMut<UiReceiver>,
    mut client: ResMut<bevy_quinnet::client::QuinnetClient>,
) {
    use bevy_quinnet::client::certificate::CertificateVerificationMode;
    use bevy_quinnet::client::connection::ClientEndpointConfiguration;
    use bevy_quinnet::shared::channels::ChannelsConfiguration;

    while let Ok(cmd) = ui_rx.try_recv() {
        match cmd {
            UiCommand::Host { server_path: _ } => {
                // Start the server in-process on a background thread
                let _ = std::thread::spawn(|| {
                    godot_print!("Starting in-process server...");
                    crate::server::create_server();
                });

                // Then connect the client to the local server
                let _ = client.open_connection(
                    ClientEndpointConfiguration::from_strings("0.0.0.0:6000", "0.0.0.0:0").unwrap(),
                    CertificateVerificationMode::SkipVerification,
                    ChannelsConfiguration::default(),
                );
            }
            UiCommand::Connect => {
                let _ = client.open_connection(
                    ClientEndpointConfiguration::from_strings("0.0.0.0:6000", "0.0.0.0:0").unwrap(),
                    CertificateVerificationMode::SkipVerification,
                    ChannelsConfiguration::default(),
                );
            }
        }
    }
}
