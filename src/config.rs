#[derive(Debug, serde::Deserialize)]
pub struct Config {
    pub mqtt_broker_uri: String,
    pub mqtt_broker_port: u16,
    pub session_expiry_interval: u16,

    pub mqtt_username: Option<String>,
    pub mqtt_password: Option<String>,

    pub notify_on_startup: Option<String>,

    pub ignore_retained: bool,

    pub message_timeout_millis: u16,

    pub mappings: Vec<Mapping>,
}

#[derive(Debug, serde::Deserialize)]
pub struct Mapping {
    pub topic: String,

    pub actions: Vec<Action>,
}

#[derive(Debug, serde::Deserialize)]
pub enum Action {
    OnValueEqSay { value: String, say: String },
    OnValueNeSay { value: String, say: String },
}

impl Action {
    pub fn is_applicable(&self, message_text: &str) -> bool {
        match self {
            Action::OnValueEqSay { value, .. } => value == message_text,
            Action::OnValueNeSay { value, .. } => value != message_text,
        }
    }

    pub fn say(&self) -> &str {
        match self {
            Action::OnValueEqSay { say, .. } => say,
            Action::OnValueNeSay { say, .. } => say,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Action;
    use super::Mapping;

    #[test]
    fn test_deser_mapping_on_value_eq_say() {
        let config = serde_json::json!({
            "topic": "foo",
            "actions": [{
                "OnValueEqSay": {
                    "value": "bar",
                    "say": "baz"
                }
            }]
        });

        let mapping: Mapping = serde_json::from_value(config).unwrap();
        assert_eq!(mapping.topic, "foo");

        let Action::OnValueEqSay { ref value, ref say } = mapping.actions[0] else {
            panic!("Did expect OnValueEqSay, got: {:?}", mapping.actions);
        };

        assert_eq!(value, "bar");
        assert_eq!(say, "baz");
    }

    #[test]
    fn test_deser_mapping_on_value_ne_say() {
        let config = serde_json::json!({
            "topic": "foo",
            "actions": [{
                "OnValueNeSay": {
                    "value": "bar",
                    "say": "baz"
                }
            }]
        });

        let mapping: Mapping = serde_json::from_value(config).unwrap();
        assert_eq!(mapping.topic, "foo");

        let Action::OnValueNeSay { ref value, ref say } = mapping.actions[0] else {
            panic!("Did expect OnValueNeSay, got: {:?}", mapping.actions);
        };

        assert_eq!(value, "bar");
        assert_eq!(say, "baz");
    }
}
