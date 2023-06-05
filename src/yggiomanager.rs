use rumqttc::{self, AsyncClient, Event, EventLoop, Incoming, MqttOptions, Packet, QoS, Transport};
use rustls::ClientConfig;
use std::error::Error;

const MQTT_BROKER_URL: &str = "mqtt.yggio.sifis-home.eu";
const MQTT_PORT: u16 = 8883;

const YGGIO_API_URL: &str = "https://yggio.sifis-home.eu/api/";

const USERNAME: &str = "myhome-administrator";
const PASSWORD: &str = "sfshm1!";

// EMULATED-----
const MQTT_USER: &str = "sifishome-admin";
const MQTT_PASSWORD: &str = "sifishome-admin";
const BASIC_CREDENTIALS_SET_ID: &str = "6463ab4545b7d8d57adba603";
// -----

// PHYSICAL ----

const BASIC_CREDENTIALS_SET_ID_PHYSICAL: &str = "647da69774dabeee5bfaf8e2";
const MQTT_USER_PHYSICAL : &str = "physical";
const MQTT_PASSWORD_PHYSICAL : &str = "physical";
// ---

const MQTT_PUBLISH_PREFIX: &str = "yggio/generic/v2/";

// this is the mqtt topic from which we can receive updates from all the iotNodes present in Yggio
const MQTT_SUBSCRIBE_TOPIC: &str =
    "yggio/output/v2/6335585ffaf1370008dec0fc/iotnode/64144eab0b6304ad3bdd713c";

pub enum YggioEvent {
    Connected,
    Disconnected,
    GotMessage(serde_json::Value),
    //GotVolatileMessage,
    None,
}

pub struct YggioManager {
    pub client: AsyncClient,
    pub event_loop: EventLoop,
    pub connected: bool,
    pub token: String,
    pub testbed_type: String
}


impl YggioManager {
    pub fn new(testbed_type: &str) -> Result<Self, Box<dyn Error>> {
        let testbed_type = testbed_type.to_owned();
        let mut mqttoptions = MqttOptions::new("sifis-dht-client", MQTT_BROKER_URL, MQTT_PORT);

        mqttoptions.set_keep_alive(std::time::Duration::from_secs(5));

        if testbed_type == "emulated" {
            mqttoptions.set_credentials(MQTT_USER, MQTT_PASSWORD);
        }

        if testbed_type == "physical" {
            mqttoptions.set_credentials(MQTT_USER_PHYSICAL, MQTT_PASSWORD_PHYSICAL);
        }


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

        let token = String::from("");
        Ok(YggioManager {
            client,
            event_loop,
            connected,
            token,
            testbed_type
        })
    }

    pub async fn get_auth_token(&self) -> Result<String, Box<dyn Error>> {
        let cred_message = serde_json::json!({"username": USERNAME, "password": PASSWORD});

        let client = reqwest::Client::new();
        let res = client
            .post(YGGIO_API_URL.to_owned() + "auth/local")
            .json(&cred_message)
            .send()
            .await;

        if let Ok(res) = res {
            if res.status() == reqwest::StatusCode::OK {
                if let Ok(body_text) = res.text().await {
                    let body: serde_json::Value = serde_json::from_str(&body_text)?;
                    if let Some(token) = body.get("token") {
                        let token = token.as_str().unwrap();
                        return Ok(token.to_owned());
                    }
                }
            }
        }

        Err("error".into())
    }

    pub async fn reserve_mqtt_topic(
        &self,
        token: &str,
        topic_name: &str,
        topic_uuid: &str,
    ) -> Result<(), Box<dyn Error>> {
        let client = reqwest::Client::new();


        let mut reserved_topic_message = serde_json::json!(
            {
                "topic": MQTT_PUBLISH_PREFIX.to_owned() + topic_name + "-" + topic_uuid + "-" + &self.testbed_type,
                "basicCredentialsSetId": BASIC_CREDENTIALS_SET_ID
            });


        if self.testbed_type == "physical" {
            reserved_topic_message = serde_json::json!(
            {
                "topic": MQTT_PUBLISH_PREFIX.to_owned() + topic_name + "-" + topic_uuid + "-" + &self.testbed_type,
                "basicCredentialsSetId": BASIC_CREDENTIALS_SET_ID_PHYSICAL
            });
        }


        println!(
            "Reserving MQTT TOPIC: {}",
            MQTT_PUBLISH_PREFIX.to_owned() + topic_name + "-" + topic_uuid + "-" + &self.testbed_type
        );

        println!("for ");

        if self.testbed_type == "emulated" {
            println!("{}", BASIC_CREDENTIALS_SET_ID);
        }

        if self.testbed_type == "physical" {
            println!("{}", BASIC_CREDENTIALS_SET_ID_PHYSICAL);
        }



        let _res = client
            .post(YGGIO_API_URL.to_owned() + "reserved-mqtt-topics")
            .header("Content-type", "application/json")
            .bearer_auth(token)
            .json(&reserved_topic_message)
            .send()
            .await;

        if let Ok(r) = _res {
            let status = r.status().clone();
            let text = r.text().await.unwrap();
            println!("{} {}", status, text);
        }

        Ok(())
    }

    pub async fn publish_on_mqtt(
        &mut self,
        topic_name: &str,
        topic_uuid: &str,
        payload_string: String,
    ) {
        let token = self.get_auth_token().await;

        let topic_name = topic_name.replace("_", "");

        if let Ok(token) = token {
            let _ret = self
                .reserve_mqtt_topic(&token, &topic_name, topic_uuid)
                .await;

            if self.connected {
                let mqtt_topic_string = MQTT_PUBLISH_PREFIX.to_owned() + &topic_name + "-" + topic_uuid + "-" + &self.testbed_type;
                if let Ok(_) = self.client
                    .publish(
                        mqtt_topic_string,
                        QoS::AtMostOnce,
                        false,
                        payload_string.into_bytes(),
                    )
                    .await {
                    println!("Pub on MQTT");
                }
            }
        } else {
            println!("No token");
        }
    }

    pub async fn event_loop(&mut self) -> YggioEvent {
        let event = self.event_loop.poll().await;
        match event {
            Ok(Event::Incoming(Packet::ConnAck(_c))) => {
                println!("Connected to the broker");
                self.connected = true;

/*                self.client
                    .subscribe(MQTT_SUBSCRIBE_TOPIC, QoS::AtMostOnce)
                    .await
                    .unwrap();*/

                YggioEvent::Connected

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

                if let Some(iot_node) = v.get("iotnode") {
                    if let Some(value) = iot_node.get("value") {
                        //println!("{value}");
                        return YggioEvent::GotMessage(value.to_owned());
                    }
                }

                YggioEvent::None
            }
            Ok(Event::Incoming(i)) => {
                println!("Incoming = {i:?}");
                YggioEvent::None
            }
            Ok(Event::Outgoing(o)) => {
                println!("Outgoing = {o:?}");
                YggioEvent::None
            }
            Err(e) => {
                println!("Error = {:?}", e);
                self.connected = false;
                YggioEvent::Disconnected
            }
        }
    }
}
