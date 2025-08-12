use bevy::prelude::*;
use godot::{
    classes::{IRichTextLabel, ITextEdit, RichTextLabel, TextEdit},
    prelude::*,
};
use godot_bevy::prelude::*;
use tokio::sync::mpsc::Sender;

#[derive(Component, Default)]
pub struct Chat {
    pub messages: Vec<String>,
}

#[derive(Component, Default)]
pub struct ChatInput {
    pub sender: Option<Sender<String>>,
}

fn gd_arr_to_rust(arr: PackedStringArray) -> Vec<String> {
    arr.as_slice().iter().map(|s| s.to_string()).collect()
}

#[derive(GodotClass, BevyBundle)]
#[class(base=RichTextLabel)]
#[bevy_bundle((Chat {messages: messages}))]
pub struct ChatNode {
    base: Base<RichTextLabel>,
    #[export]
    #[bevy_bundle(transform_with = "gd_arr_to_rust")]
    messages: PackedStringArray,
}

#[derive(GodotClass, BevyBundle)]
#[class(base=TextEdit)]
#[bevy_bundle((ChatInput {sender: sender}))]
pub struct ChatInputNode {
    base: Base<TextEdit>,
    #[bevy_bundle]
    sender: Option<Sender<String>>,
}

#[godot_api]
impl IRichTextLabel for ChatNode {
    fn init(base: Base<RichTextLabel>) -> Self {
        Self {
            base,
            messages: PackedStringArray::new(),
        }
    }
}

#[godot_api]
impl ITextEdit for ChatInputNode {
    fn init(base: Base<TextEdit>) -> Self {
        Self { base, sender: None }
    }
}

#[main_thread_system]
pub fn read_chat_messages(
    mut query: Query<(Entity, &mut GodotNodeHandle, &mut ChatInput), With<TextEditMarker>>,
    mut events: EventReader<ActionInput>,
) {
    for (_, mut handle, chat_input) in query.iter_mut() {
        let mut chat_input_node = handle.get::<ChatInputNode>();
        for event in events.read() {
            if event.action.as_str() == "ui_text_submit" {
                let text = chat_input_node.get_text().to_string();
                if text.is_empty() {
                    continue;
                }
                if let Some(sender) = &chat_input.sender {
                    godot_print!("Sending message: {}", text);
                    sender.try_send(text.trim_end().to_string()).unwrap();
                }
                chat_input_node.set_text("");
            }
        }
    }
}
