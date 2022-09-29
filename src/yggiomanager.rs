use rumqttc::{self, AsyncClient, Event, EventLoop, Incoming, MqttOptions, Packet, QoS, Transport};
use rustls::ClientConfig;
use std::error::Error;

const MQTT_BROKER_URL: &str = "mqtt.yggio.sifis-home.eu";
const MQTT_PORT: u16 = 8883;
const MQTT_USER: &str = "sifishome";
const MQTT_PASSWORD: &str = "sifishome";
const MQTT_PUBLISH_PREFIX: &str = "yggio/generic/v2/sifisdht";
const MQTT_SUBSCRIBE_TOPIC: &str = "yggio/output/v2/6335585ffaf1370008dec0fc/#";

pub enum YggioEvent {
    Connected,
    Disconnected,
    GotMessage,
    GotVolatileMessage,
    None,
}

pub struct YggioManager {
    pub client: AsyncClient,
    pub event_loop: EventLoop,
    pub connected: bool,
}

impl YggioManager {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let mut mqttoptions = MqttOptions::new("sifis-dht-client", MQTT_BROKER_URL, MQTT_PORT);

        mqttoptions.set_keep_alive(std::time::Duration::from_secs(5));

        mqttoptions.set_credentials(MQTT_USER, MQTT_PASSWORD);

        // Use rustls-native-certs to load root certificates from the operating system.
        let mut root_cert_store = rustls::RootCertStore::empty();

        for cert in rustls_native_certs::load_native_certs().expect("could not load platform certs")
        {
            root_cert_store.add(&rustls::Certificate(cert.0))?;
        }

        let client_config = ClientConfig::builder()
            .with_safe_defaults()
            .with_root_certificates(root_cert_store)
            .with_no_client_auth();

        mqttoptions.set_transport(Transport::tls_with_config(client_config.into()));

        let (client, event_loop) = AsyncClient::new(mqttoptions, 10);

        let connected = false;

        Ok(YggioManager {
            client,
            event_loop,
            connected,
        })
    }

    pub async fn publish_on_mqtt(
        &mut self,
        topic_name: &str,
        topic_uuid: &str,
        payload_string: String,
    ) {
        if self.connected {
            let _ret = self
                .client
                .publish(
                    MQTT_PUBLISH_PREFIX.to_owned() + "/" + topic_name + "/" + topic_uuid,
                    QoS::AtMostOnce,
                    false,
                    payload_string.into_bytes(),
                )
                .await
                .unwrap();
        }
    }

    pub async fn event_loop(&mut self) -> YggioEvent {
        let event = self.event_loop.poll().await;
        match event {
            Ok(Event::Incoming(Packet::ConnAck(_c))) => {
                println!("Connected to the broker");
                self.connected = true;

                let mut _ret = self
                    .client
                    .subscribe(MQTT_SUBSCRIBE_TOPIC, QoS::AtMostOnce)
                    .await
                    .unwrap();

                return YggioEvent::Connected;

                /*
                // by publishing a fake message we force Yggio to publish the current saved state
                let fake_message = String::from("{}").into_bytes();
                _ret = self
                    .client
                    .publish(MQTT_PUBLISH_PREFIX, QoS::AtMostOnce, false, fake_message)
                    .await
                    .unwrap();
                 */
            }
            Ok(Event::Incoming(Incoming::Publish(p))) => {
                println!(
                    "Topic: {}, Payload: {:?}, Retained {:?}",
                    p.topic, p.payload, p.retain
                );

                let payload_string =
                    String::from_utf8_lossy(p.payload.to_vec().as_slice()).to_string();

                let v: serde_json::Value = serde_json::from_str(&payload_string).unwrap();

                if let Some(iotNode) = v.get("iotnode") {
                    if let Some(value) = iotNode.get("value") {
                        println!("{}", value);
                    }
                }
                return YggioEvent::GotMessage;
            }
            Ok(Event::Incoming(i)) => {
                println!("Incoming = {:?}", i);
                return YggioEvent::None;
            }
            Ok(Event::Outgoing(o)) => {
                println!("Outgoing = {:?}", o);
                return YggioEvent::None;
            }
            Err(e) => {
                //println!("Error = {:?}", e);
                self.connected = false;
                return YggioEvent::Disconnected;
            }
        }
    }
}
