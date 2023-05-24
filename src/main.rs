mod yggiomanager;
use crate::yggiomanager::{YggioEvent, YggioManager};
use sifis_dht::domocache::{DomoCache, DomoEvent};
use std::error::Error;
use clap::Parser;
use sifis_config::{Cache, ConfigParser};
use serde::{Deserialize, Serialize};

#[derive(Parser, Debug, Serialize, Deserialize)]
struct DhtToMqtt {
    #[clap(flatten)]
    pub cache: Cache,
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

    let mut sifis_cache = sifis_dht::domocache::DomoCache::new(
  opt.cache
    )
    .await?;

    let mut yggio_manager = yggiomanager::YggioManager::new()?;

    loop {
        tokio::select! {

            event = yggio_manager.event_loop() => {

                if let YggioEvent::Connected = event {
                        // publish entire cache on MQTT
                        println!("PUB ENTIRE CACHE ON MQTT");
                        publish_all_on_mqtt(&mut sifis_cache, &mut yggio_manager).await;
                }

                if let YggioEvent::GotMessage(m) = event {
                    println!("Yggio command received");
                    println!("{}", m);
                }
            }

            message = sifis_cache.cache_event_loop() => {

                match message {
                    Ok(m) => {
                        if let DomoEvent::PersistentData(m) = m {
                                println!("Persistent {m}");

                                // publish persistent message on Yggio
                                let m2 = serde_json::to_string(&m.value).unwrap();

                                yggio_manager.publish_on_mqtt(&m.topic_name, &m.topic_uuid, m2).await;
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

async fn publish_all_on_mqtt(
    sifis_cache: &mut DomoCache,
    yggio_manager: &mut YggioManager,
) {
    for (topic_name, topic_name_map) in sifis_cache.cache.iter() {
        for (topic_uuid, cache_element) in topic_name_map.iter() {
            let m = serde_json::to_string(cache_element).unwrap();
            yggio_manager
                .publish_on_mqtt(topic_name, topic_uuid, m)
                .await;
        }
    }
}
