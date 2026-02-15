// src-tauri/src/main.rs
use rumqttc::{MqttOptions, Client, QoS, Event, Packet};
use serde::Serialize;
use std::{thread, time::Duration, sync::Mutex};
// Note the use of Emitter for Tauri v2
use tauri::{Emitter, Manager, State};

// Shared state to hold the MQTT client so we can publish from JS
struct MqttHandle(Mutex<Client>);

#[derive(Clone, Serialize)]
struct Payload {
    topic: String,
    message: String,
}

// Command to publish to MQTT from the Frontend
#[tauri::command]
fn publish_mqtt(topic: String, payload: String, state: State<'_, MqttHandle>) {
    let mut client = state.0.lock().unwrap();
    // Publish to the topic provided by JS (e.g., NODE_1/output)
    if let Err(e) = client.publish(topic, QoS::AtLeastOnce, false, payload) {
        eprintln!("Failed to publish MQTT message: {:?}", e);
    }
}

fn main() {
    // 1. MQTT Configuration
    // Replace "127.0.0.1" with your actual broker IP if it's not local
    let mut mqttoptions = MqttOptions::new("banana_pi_hmi", "140.245.7.35", 1883);
    mqttoptions.set_keep_alive(Duration::from_secs(5));

    let (client, mut connection) = Client::new(mqttoptions, 10);

    tauri::Builder::default()
        .manage(MqttHandle(Mutex::new(client)))
        .setup(|app| {
            let handle = app.handle().clone();
            
            // 2. Spawn the MQTT Listener Thread
            thread::spawn(move || {
                for notification in connection.iter() {
                    match notification {
                        Ok(Event::Incoming(Packet::Publish(p))) => {
                            let msg = String::from_utf8_lossy(&p.payload).to_string();
                            // In Tauri v2, emit_all is replaced by emit
                            let _ = handle.emit("mqtt-data", Payload { 
                                topic: p.topic, 
                                message: msg 
                            });
                        }
                        Err(e) => {
                            eprintln!("MQTT Connection Error: {:?}", e);
                            thread::sleep(Duration::from_secs(1));
                        }
                        _ => {}
                    }
                }
            });

            // 3. Subscription (Listen for all node inputs)
            // Using a scoped block to release the lock immediately
            {
                let state = app.state::<MqttHandle>();
                let mut client_lock = state.0.lock().unwrap();
                // Subscribes to any topic ending in /input (e.g., NODE_01/input)
                client_lock.subscribe("+/input", QoS::AtMostOnce).expect("Subscription failed");
            }
            
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![publish_mqtt])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
