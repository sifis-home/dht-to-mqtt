mod utils;
mod yggiomanager;
use crate::yggiomanager::{YggioEvent, YggioManager};
use sifis_dht::domocache::{DomoCache, DomoEvent};
use sifis_dht::domopersistentstorage::SqliteStorage;
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();
    let dht_shared_key = "5a52aafb2a44ff5c360d4dc04e4a792e28637da07b96072a2f0a5ea5286f2738";

    let storage = sifis_dht::domopersistentstorage::SqliteStorage::new("/tmp/mqtt.sqlite", true);

    let mut pkcs8_der = utils::generate_rsa_key().1;

    let local_key = libp2p::identity::Keypair::rsa_from_pkcs8(&mut pkcs8_der)
        .map_err(|e| format!("Couldn't load key: {e:?}"))?;

    let mut sifis_cache = sifis_dht::domocache::DomoCache::new(
        true,
        storage,
        dht_shared_key.to_owned(),
        local_key,
        false,
    )
    .await;

    let mut yggio_manager = yggiomanager::YggioManager::new()?;

    loop {
        tokio::select! {

            event = yggio_manager.event_loop() => {

                match event {
                    YggioEvent::Connected => {
                        // publish entire cache on MQTT
                        println!("PUB ENTIRE CACHE ON MQTT");
                        publish_all_on_mqtt(&mut sifis_cache, &mut yggio_manager).await;
                    }
                    _ => {}
                }
            }

            message = sifis_cache.cache_event_loop() => {

                match message {
                    Ok(m) => {
                        match m {
                            DomoEvent::PersistentData(m) => {
                                println!("Persistent {}", m);

                                // publish persistent message on Yggio
                                let m2 = serde_json::to_string(&m).unwrap();

                                yggio_manager.publish_on_mqtt(&m.topic_name, &m.topic_uuid, m2).await;

                            },
                            _ => {
                            }
                        }
                    },
                    Err(e) => {
                        println!("{}", e);
                    }
                }

            }

        }
    }
}

async fn publish_all_on_mqtt(
    sifis_cache: &mut DomoCache<SqliteStorage>,
    yggio_manager: &mut YggioManager,
) {
    for (topic_name, topic_name_map) in sifis_cache.cache.iter() {
        for (topic_uuid, cache_element) in topic_name_map.iter() {
            let m = serde_json::to_string(cache_element).unwrap();
            yggio_manager
                .publish_on_mqtt(&topic_name, &topic_uuid, m)
                .await;
        }
    }
}
