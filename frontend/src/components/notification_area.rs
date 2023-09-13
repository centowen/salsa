use yew::prelude::*;

#[derive(Clone, Properties, PartialEq)]
pub struct NotificationAreaProps {
    pub message: String,
    pub level: NotificationLevel,
}

impl NotificationAreaProps {
    pub fn with_level(message: &str, level: NotificationLevel) -> Self {
        Self {
            message: message.to_owned(),
            level,
        }
    }

    pub fn empty() -> Self {
        Self::with_level("", NotificationLevel::None)
    }

    pub fn success(message: &str) -> Self {
        Self::with_level(message, NotificationLevel::Success)
    }

    pub fn error(message: &str) -> Self {
        Self::with_level(message, NotificationLevel::Error)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum NotificationLevel {
    None,
    Success,
    Error,
}

#[function_component(NotificationArea)]
pub fn notification_area(props: &NotificationAreaProps) -> Html {
    let NotificationAreaProps { message, level } = props.clone();
    let level_class = match level {
        NotificationLevel::None => None,
        NotificationLevel::Success => Some("success"),
        NotificationLevel::Error => Some("error"),
    };

    html! {
        <div class={classes!("notification-area", level_class)}>
            { message }
        </div>
    }
}
