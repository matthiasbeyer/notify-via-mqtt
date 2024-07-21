use clap::Parser;
use miette::IntoDiagnostic;
use rumqttc::AsyncClient;
use rumqttc::MqttOptions;
use tracing::Level;

mod cli;
mod config;

#[tokio::main(flavor = "current_thread")]
async fn main() -> miette::Result<()> {
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

    let config_str = std::fs::read_to_string(&cli.config)
        .inspect_err(|error| tracing::error!(?error, "Failed to read configuration"))
        .into_diagnostic()?;
    tracing::trace!(?config_str, "Configuration read from disk");

    let config: crate::config::Config = serde_json::from_str(&config_str)
        .inspect_err(|error| {
            tracing::error!(?error, "Failed to parse configuration");
        })
        .into_diagnostic()?;

    let mut mqttoptions = MqttOptions::new(
        "notify-via-mqtt",
        config.mqtt_broker_uri,
        config.mqtt_broker_port,
    );
    mqttoptions.set_keep_alive(std::time::Duration::from_secs(
        config.session_expiry_interval.into(),
    ));
    if let (Some(username), Some(password)) =
        (config.mqtt_username.as_ref(), config.mqtt_password.as_ref())
    {
        mqttoptions.set_credentials(username, password);
    }

    let (client, mut eventloop) = AsyncClient::new(mqttoptions, 10);

    for mapping in config.mappings.iter() {
        let () = client
            .subscribe(&mapping.topic, rumqttc::QoS::AtMostOnce)
            .await
            .inspect_err(|error| tracing::error!(?error, "Failed to subscribe"))
            .into_diagnostic()?;
    }

    if let Some(notify_on_startup) = config.notify_on_startup.as_ref().cloned() {
        tokio::task::spawn_blocking(move || {
            if let Err(error) = notify_rust::Notification::new()
                .summary("Startup")
                .body(&notify_on_startup)
                .timeout(notify_rust::Timeout::Milliseconds(
                    config.message_timeout_millis.into(),
                ))
                .show()
                .into_diagnostic()
            {
                tracing::error!(?error, "Failed to show notification");
            }
        });
    }

    loop {
        let notification = eventloop
            .poll()
            .await
            .inspect_err(|error| tracing::error!(?error, "Failed to poll from eventloop"))
            .into_diagnostic()?;

        let rumqttc::Event::Incoming(rumqttc::Incoming::Publish(rumqttc::mqttbytes::v4::Publish {
            payload,
            topic,
            ..
        })) = notification
        else {
            continue;
        };

        let message_text = match String::from_utf8(payload.to_vec()) {
            Ok(text) => {
                tracing::info!("Received message: '{text}'");
                text
            }
            Err(error) => {
                tracing::error!(?error, payload = ?payload, "Invalid UTF8 received");
                continue;
            }
        };

        let message_text = config
            .mappings
            .iter()
            .filter(|mapping| mapping.topic == topic)
            .find(|mapping| mapping.action.is_applicable(&message_text))
            .map(|mapping| mapping.action.say().to_string())
            .unwrap_or_else(|| format!("Received message: {message_text}"));

        tokio::task::spawn_blocking(move || {
            if let Err(error) = notify_rust::Notification::new()
                .summary("MQTT Notification")
                .body(&message_text)
                .timeout(notify_rust::Timeout::Milliseconds(
                    config.message_timeout_millis.into(),
                ))
                .show()
                .into_diagnostic()
            {
                tracing::error!(?error, "Failed to show notification");
            }
        });
    }
}
