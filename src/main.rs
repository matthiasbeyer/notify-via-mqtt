use clap::Parser;
use futures::stream::StreamExt;
use tracing::{Instrument, Level};

mod cli;
mod config;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let cli = crate::cli::Cli::parse();
    tracing_subscriber::fmt()
        .pretty()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_max_level({
            match (cli.trace, cli.debug, cli.verbose) {
                (true, _, _) => Level::TRACE,
                (false, true, _) => Level::DEBUG,
                (false, false, true) => Level::INFO,
                _ => Level::WARN,
            }
        })
        .init();
    tracing::debug!(?cli);

    let config_str = tokio::fs::read_to_string(&cli.config).await.unwrap();
    tracing::trace!(?config_str, "Configuration read from disk");

    let config: crate::config::Config = toml::from_str(&config_str).unwrap();
    tracing::trace!(?config, "Configuration parsed");

    let connect_str = format!(
        "tcp://{}:{}",
        config.mqtt_broker_uri, config.mqtt_broker_port
    );
    let client = match paho_mqtt::AsyncClient::new(connect_str) {
        Ok(client) => {
            tracing::debug!("Created MQTT client");
            client
        }
        Err(error) => {
            tracing::error!(?error, "Error creating the MQTT client: {error:?}");
            std::process::exit(1)
        }
    };

    let mut conn_opts =
        paho_mqtt::ConnectOptionsBuilder::with_mqtt_version(paho_mqtt::MQTT_VERSION_5);
    conn_opts
        .properties(paho_mqtt::properties![paho_mqtt::PropertyCode::SessionExpiryInterval => config.session_expiry_interval as u64]);
    conn_opts.clean_session(true);

    if let (Some(username), Some(password)) =
        (config.mqtt_username.as_ref(), config.mqtt_password.as_ref())
    {
        conn_opts.user_name(username);
        conn_opts.password(password);
    }

    let conn_opts = conn_opts.finalize();

    // Connect and wait for it to complete or fail
    match client.connect(conn_opts).await {
        Err(e) => {
            tracing::error!("Unable to connect: {e:?}");
            std::process::exit(1)
        }

        Ok(_) => {
            tracing::info!("MQTT connected");
        }
    }

    let topics = config
        .mappings
        .iter()
        .map(|mapping| mapping.topic.to_string())
        .collect::<Vec<String>>();
    let topics_qos = vec![paho_mqtt::QOS_1; topics.len()];

    let sub_opts = vec![paho_mqtt::SubscribeOptions::with_retain_as_published(); topics.len()];

    let mut client = client; // rebind mutably
    let mut stream = client.get_stream(25);

    match client
        .subscribe_many_with_options(&topics, &topics_qos, &sub_opts, None)
        .instrument(tracing::debug_span!("mqtt.subscribing"))
        .await
    {
        Err(e) => {
            tracing::error!("Failed to subscribe: {e:?}");
            std::process::exit(1)
        }

        Ok(_) => {
            tracing::info!(?topics, "MQTT subscribed");
        }
    }

    while let Some(msg_opt) = stream.next().await {
        if let Some(msg) = msg_opt {
            let message_text = match String::from_utf8(msg.payload().to_vec()) {
                Ok(text) => {
                    tracing::info!("Received message: '{text}'");
                    text
                }
                Err(error) => {
                    tracing::error!(?error, payload = ?msg.payload(), "Invalid UTF8 received");
                    continue;
                }
            };

            let message_text = config
                .mappings
                .iter()
                .filter(|mapping| mapping.topic == msg.topic())
                .find(|mapping| mapping.action.is_applicable(&message_text))
                .map(|mapping| mapping.action.say().to_string())
                .unwrap_or_else(|| format!("Received message: {message_text}"));

            tokio::task::spawn_blocking(move || {
                notify_rust::Notification::new()
                    .summary("MQTT Notification")
                    .body(&message_text)
                    .timeout(notify_rust::Timeout::Milliseconds(
                        config.message_timeout_millis.into(),
                    ))
                    .show()
                    .unwrap();
            });
        } else {
            // A "None" means we were disconnected. Try to reconnect...
            tracing::info!("Lost connection. Attempting reconnect.");
            while let Err(error) = client.reconnect().await {
                tracing::error!(?error, "Error reconnecting");
                tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
            }
        }
    }

    tracing::info!("Finished");
}
