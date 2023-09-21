use yew::prelude::*;

#[function_component(WelcomePage)]
pub fn welcome_page() -> Html {
    html! {
        <div class="section dark welcome">
            // <h1>{ "Observe the Milky Way" }</h1>
            <h1>{ "Radio astronomy in your browser" }</h1>
            <div><p>{ "The Salsa telescopes are available right here in your
            browser. Do real radio observations of our home, the Milky Way."
            }</p></div>
            // "Try it now!" Button which leads to the observation page?
        </div>
    }
}
