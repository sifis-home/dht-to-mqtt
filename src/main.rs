mod yggiomanager;
use crate::yggiomanager::{YggioEvent, YggioManager};
use chrono::prelude::*;
use clap::Parser;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sifis_config::{Cache, ConfigParser};
use sifis_dht::domocache::{DomoCache, DomoEvent};
use std::error::Error;

#[derive(Parser, Debug, Serialize, Deserialize)]
struct DhtToMqtt {
    #[clap(flatten)]
    pub cache: Cache,

    #[arg(long, default_value = "emulated")]
    pub testbed_type: String,
}

#[derive(Parser, Debug, Serialize, Deserialize)]
struct Opt {
    #[clap(flatten)]
    dht_to_mqtt: DhtToMqtt,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let opt = ConfigParser::<Opt>::new()
        .with_config_path("/etc/domo/dht_to_mqtt.toml")
        .parse();

    let opt = opt.dht_to_mqtt;

    let mut sifis_cache = sifis_dht::domocache::DomoCache::new(opt.cache).await?;

    let mut yggio_manager = yggiomanager::YggioManager::new(&opt.testbed_type)?;

    loop {
        println!("Main select");
        tokio::select! {

            event = yggio_manager.event_loop() => {

                if let YggioEvent::Connected = event {
                        // publish entire cache on MQTT
                        println!("PUB ENTIRE CACHE ON MQTT");
                        //publish_all_on_mqtt(&mut sifis_cache, &mut yggio_manager).await;
                }

                if let YggioEvent::GotMessage(m) = event {
                    println!("Yggio command received");
                    println!("{}", m);

                    if let Some(topic_name) = m.get("topic_name") {
                        if let Some(topic_uuid) = m.get("topic_uuid") {

                            if let Some(coap_client_commant) = m.get("coap_client_command") {
                                println!("Command for coap client received");

                                if let Some(coap_message) = coap_client_commant.as_object() {
                                    if let Some(message) = coap_message.get("message") {
                                        println!("Message {}", message);
                                        if let Some(topic) = coap_message.get("topic") {
                                            println!("Topic {}", topic);

                                            let current_time = Utc::now().to_string();

                                            let command_message = json!(
                                            {
                                                "timestamp": current_time,
                                                "message": message,
                                                "topic": topic
                                            });

                                            sifis_cache.pub_value(command_message).await;

                                        }
                                    }
                                }


                            }

                            if let Some(desired_state) = m.get("desired_state") {

                                let topic_name = topic_name.as_str().unwrap();

                                let topic_uuid = topic_uuid.as_str().unwrap();

                                let current_time = Utc::now().to_string();

                                let on = desired_state.as_bool().unwrap();

                                let command_message = json!(
                                    {
                                    "timestamp": current_time,
                                        "command": {
                                            "command_type": "turn_command",
                                            "value": {
                                                "topic_name": topic_name,
                                                "topic_uuid": topic_uuid,
                                                "desired_state": on
                                            }
                                        }
                                    });

                                sifis_cache.pub_value(command_message).await;

                            }
                        }
                    }


                }
            }

            message = sifis_cache.cache_event_loop() => {

                match message {
                    Ok(m) => {
                        match m {
                            DomoEvent::PersistentData(m) => {
                                println!("Persistent {m}");

                                // publish persistent message on Yggio
                                let m2 = serde_json::to_string(&m).unwrap();

                                if m.topic_name == "domo_light" ||
                                   m.topic_name == "domo_window_sensor" ||
                                   m.topic_name == "shelly_1pm" ||
                                   m.topic_name == "domo_pir_sensor" ||
                                   m.topic_name == "domo_bistable_button" ||
                                   m.topic_name == "sifis_coap_client"
                                {
                                    yggio_manager.publish_on_mqtt(&m.topic_name, &m.topic_uuid, m2).await;
                                }
                            },
                            DomoEvent::VolatileData(m) => {
                                if let Some(topic) = m.get("topic"){
                                    if let Some(topic) = topic.as_str() {
                                        if topic == "output_dev2" {
                                            println!("Volatile");
                                            let m2 = serde_json::to_string(&m).unwrap();
                                            yggio_manager.publish_on_mqtt("volatile_logger", "volatile_uuid", m2).await;
                                        }
                                    }

                                }

                            },
                            _ => {

                            }
                        }

                    },
                    Err(e) => {
                        println!("{e}");
                    }
                }

            }

        }
    }
}

async fn publish_all_on_mqtt(sifis_cache: &mut DomoCache, yggio_manager: &mut YggioManager) {
    for (topic_name, topic_name_map) in sifis_cache.cache.iter() {
        for (topic_uuid, cache_element) in topic_name_map.iter() {
            let m = serde_json::to_string(cache_element).unwrap();
            yggio_manager
                .publish_on_mqtt(topic_name, topic_uuid, m)
                .await;
        }
    }

    println!("PUBLISH ALL DONE");
}
