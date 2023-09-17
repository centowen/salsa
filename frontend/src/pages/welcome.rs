use yew::prelude::*;

#[function_component(WelcomePage)]
pub fn welcome_page() -> Html {
    html! {
        <div class="welcome">
            <h1>{ "Observe the galaxy" }</h1>
            <h3>{ "with a real radio telescope" }</h3>
            // "Try it now!" Button which leads to the observation page?
        </div>
    }
}
