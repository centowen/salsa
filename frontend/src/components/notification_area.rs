use yew::prelude::*;

#[derive(Clone, Properties, PartialEq)]
pub struct NotificationAreaProps {
    pub message: String,
    pub level: NotificationLevel,
}

impl NotificationAreaProps {
    pub fn empty() -> Self {
        Self::message("")
    }

    pub fn with_level(message: &str, level: NotificationLevel) -> Self {
        Self {
            message: message.to_owned(),
            level,
        }
    }

    pub fn message(message: &str) -> Self {
        Self::with_level(message, NotificationLevel::Message)
    }

    pub fn success(message: &str) -> Self {
        Self::with_level(message, NotificationLevel::Success)
    }

    pub fn warning(message: &str) -> Self {
        Self::with_level(message, NotificationLevel::Warning)
    }

    pub fn error(message: &str) -> Self {
        Self::with_level(message, NotificationLevel::Error)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum NotificationLevel {
    Message,
    Success,
    Warning,
    Error,
}

#[function_component(NotificationArea)]
pub fn notification_area(props: &NotificationAreaProps) -> Html {
    let NotificationAreaProps { message, level } = props.clone();
    let level_class = match level {
        NotificationLevel::Message => "message",
        NotificationLevel::Success => "success",
        NotificationLevel::Warning => "warning",
        NotificationLevel::Error => "error",
    };

    html! {
        <div class={classes!("notification-area", level_class)}>
            { message }
        </div>
    }
}
