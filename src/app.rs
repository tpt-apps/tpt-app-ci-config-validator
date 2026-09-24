// DOM UI. Phase 0: hello-world proof of the hub contract; the real
// upload/paste + findings UI lands in Phase 2.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
pub fn mount_app(container: &web_sys::Element) -> Result<(), JsValue> {
    use wasm_bindgen::JsCast;

    let window = web_sys::window().expect("no window");
    let document = window.document().expect("no document");

    let heading = document.create_element("h1")?;
    heading.set_text_content(Some("TPT CI Config Validator"));
    container.append_child(&heading)?;

    let para = document.create_element("p")?;
    para.set_text_content(Some(
        "Validate Kubernetes, Terraform, and CI config files against built-in schema rules — free, in your browser, one file at a time.",
    ));
    container.append_child(&para)?;

    let badge = document
        .create_element("div")?
        .dyn_into::<web_sys::HtmlElement>()?;
    badge.set_text_content(Some("scaffold OK — Phase 0"));
    container.append_child(&badge)?;

    Ok(())
}
