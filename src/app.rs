// DOM UI — free in-browser edition.
//
// Structure follows the appfront pattern proven in tpt-app-fea-lite:
//   1. CSS injected once into <head>; all styling via `cicfg-` classes.
//   2. The editable shell (textarea, buttons) mounts ONCE and never
//      re-renders, so typing never loses focus — values are read straight
//      from the DOM when Validate runs.
//   3. Only the results placeholder re-renders, via
//      `tpt_appfront_dom::render(view)` watching the result signal.
//   4. There is no file-input node kind, so the upload button drives a raw
//      hidden `<input type=file>` + FileReader.
//
// Free edition: one file at a time, built-in schema packs only. The Pro
// build (`--features pro`) compiles in multi-file batch validation and the
// custom-rule loader; the free build shows the upsell card instead.

#[cfg(target_arch = "wasm32")]
use std::rc::Rc;

#[cfg(target_arch = "wasm32")]
use tpt_appfront_core::{ContainerBuilder, Signal, UITree};
#[cfg(target_arch = "wasm32")]
use tpt_config_engine::{Report, Severity};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::{JsCast, JsValue};

#[cfg(target_arch = "wasm32")]
type Reports = Vec<Report>;

#[cfg(target_arch = "wasm32")]
#[derive(Clone)]
enum Msg {
    /// Validate whatever is in the textarea right now.
    Validate,
    /// A file was read; swap it into the textarea and validate. (Free build
    /// only — the pro build batches through `BatchLoaded`.)
    #[cfg(not(feature = "pro"))]
    FileLoaded { name: String, text: String },
    /// Pro: several files picked at once; validate all, render one card per
    /// document.
    #[cfg(feature = "pro")]
    BatchLoaded(Vec<(String, String)>),
    /// Load one of the built-in samples.
    Sample(&'static str, &'static str),
    Clear,
}

#[cfg(target_arch = "wasm32")]
struct AppState {
    reports: Signal<Reports>,
    has_run: Signal<bool>,
}

#[cfg(target_arch = "wasm32")]
pub fn mount_app(container: &web_sys::Element) -> Result<(), JsValue> {
    let window = web_sys::window().expect("no window");
    let document = window.document().expect("no document");

    upsert_style(&document, "cicfg-styles", BASE_CSS);
    container.set_class_name("cicfg-app");

    let state = AppState {
        reports: Signal::new(Vec::new()),
        has_run: Signal::new(false),
    };

    let dispatch: Rc<dyn Fn(Msg)> = {
        let reports = state.reports.clone();
        let has_run = state.has_run.clone();
        let container = container.clone();
        Rc::new(move |msg| match msg {
            Msg::Validate => {
                let text = read_textarea(&container).unwrap_or_default();
                let name = read_filename(&container).unwrap_or_else(|| "<paste>".to_string());
                let name = if name.is_empty() { "<paste>".to_string() } else { name };
                let (custom, mut parsed) = custom_rules(&container);
                parsed.extend(tpt_config_engine::validate_source(&name, &text, custom.as_ref()));
                reports.set(parsed);
                has_run.set(true);
            }
            #[cfg(not(feature = "pro"))]
            Msg::FileLoaded { name, text } => {
                write_textarea(&container, &text);
                write_filename(&container, &name);
                let (custom, mut parsed) = custom_rules(&container);
                parsed.extend(tpt_config_engine::validate_source(&name, &text, custom.as_ref()));
                reports.set(parsed);
                has_run.set(true);
            }
            #[cfg(feature = "pro")]
            Msg::BatchLoaded(files) => {
                let (custom, mut parsed) = custom_rules(&container);
                if let Some((_, last_text)) = files.last() {
                    write_textarea(&container, last_text);
                }
                if let Some((last_name, _)) = files.last() {
                    write_filename(&container, last_name);
                }
                for (name, text) in &files {
                    parsed.extend(tpt_config_engine::validate_source(name, text, custom.as_ref()));
                }
                reports.set(parsed);
                has_run.set(true);
            }
            Msg::Sample(name, text) => {
                write_textarea(&container, text);
                write_filename(&container, name);
                let (custom, mut parsed) = custom_rules(&container);
                parsed.extend(tpt_config_engine::validate_source(name, text, custom.as_ref()));
                reports.set(parsed);
                has_run.set(true);
            }
            Msg::Clear => {
                write_textarea(&container, "");
                write_filename(&container, "");
                reports.set(Vec::new());
                has_run.set(false);
            }
        })
    };

    // 1) Static shell, mounted once (before wiring the file input: the
    // wiring looks up the shell's Upload button).
    let root = tpt_appfront_dom::mount(container, &shell_tree(), dispatch.clone())?;
    std::mem::forget(root);

    // 2) Reactive results into the placeholder. The view must produce a
    // single root child for the reconciler (`into_only_child`).
    let results_el = container
        .query_selector(".cicfg-results")?
        .ok_or_else(|| JsValue::from_str("missing .cicfg-results placeholder"))?;
    let (reports_sig, has_run_sig) = (state.reports.clone(), state.has_run.clone());
    let handle = tpt_appfront_dom::render(
        &results_el,
        Rc::new(move || results_view(&reports_sig, &has_run_sig)),
        dispatch.clone(),
    )?;
    std::mem::forget(handle);

    // 3) Hidden file input driving the Upload button (free: one file; pro:
    // `multiple` for batch).
    wire_file_input(container, &dispatch)?;

    // 4) Pro footer: the paid desktop build has no hub chrome around it, so
    // point customers back at the TPT tools page.
    #[cfg(feature = "pro")]
    mount_footer(&document, container)?;

    Ok(())
}

#[cfg(all(target_arch = "wasm32", feature = "pro"))]
const TOOLS_URL: &str = "https://tptsolutions.co.nz/tools";

/// Footer link to the tools page. In the desktop shell a plain navigation
/// would replace the app, so the click is routed through the webview's
/// `open_external` action (OS default browser); in a normal browser the
/// anchor's own `href` + `target=_blank` does the job.
#[cfg(all(target_arch = "wasm32", feature = "pro"))]
fn mount_footer(document: &web_sys::Document, container: &web_sys::Element) -> Result<(), JsValue> {
    let footer = document.create_element("div")?;
    footer.set_class_name("cicfg-footer");
    footer.append_child(&document.create_text_node("More developer tools from TPT Solutions · "))?;

    let link = document.create_element("a")?;
    link.set_attribute("href", TOOLS_URL)?;
    link.set_attribute("target", "_blank")?;
    link.set_attribute("rel", "noopener noreferrer")?;
    link.set_text_content(Some("tptsolutions.co.nz/tools"));
    footer.append_child(&link)?;
    container.append_child(&footer)?;

    let on_click: Box<dyn FnMut(web_sys::Event)> = Box::new(|e| {
        let Some(window) = web_sys::window() else { return };
        let Ok(bridge) = js_sys::Reflect::get(&window, &JsValue::from_str("__appfront")) else { return };
        if bridge.is_undefined() || bridge.is_null() {
            return;
        }
        let Ok(post) = js_sys::Reflect::get(&bridge, &JsValue::from_str("post")) else { return };
        let Ok(post) = post.dyn_into::<js_sys::Function>() else { return };
        let params = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&params, &JsValue::from_str("url"), &JsValue::from_str(TOOLS_URL));
        e.prevent_default();
        let _ = post.call2(&bridge, &JsValue::from_str("open_external"), &params);
    });
    let on_click = wasm_bindgen::closure::Closure::wrap(on_click);
    link.add_event_listener_with_callback("click", on_click.as_ref().unchecked_ref())?;
    on_click.forget();
    Ok(())
}

// ---------- static shell ----------

#[cfg(target_arch = "wasm32")]
fn shell_tree() -> UITree<Msg> {
    UITree::container(|c| {
        c.container(|h| {
            h.heading(1, "TPT CI Config Validator").class("cicfg-title");
            h.container(|p| {
                #[cfg(feature = "pro")]
                p.text("Validate Kubernetes, Terraform, and CI config files against built-in schema rules and your own custom rules. Paste a file below, or upload one or many for a batch run.");
                #[cfg(not(feature = "pro"))]
                p.text("Validate Kubernetes, Terraform, and CI config files against built-in schema rules — free, in your browser. Paste a file below or upload one.");
            })
            .class("cicfg-sub");
        })
        .class("cicfg-header");

        c.container(|io| {
            io.container(|lab| {
                lab.container(|l| { l.text("Config file"); }).class("cicfg-label");
                lab.input("")
                    .class("cicfg-filename")
                    .attr("placeholder", "file name (optional — helps format detection)")
                    .attr("spellcheck", "false");
            })
            .class("cicfg-filenamerow");
            io.textarea("")
                .class("cicfg-source")
                .attr("placeholder", "Paste a Kubernetes manifest, GitHub Actions workflow, Terraform file, or XML config here…")
                .attr("spellcheck", "false");
            io.container(|btns| {
                btns.button("Validate").class("cicfg-btn cicfg-primary").on_click(Msg::Validate);
                // No on_click in the tree: the real handler is attached in
                // wire_file_input (it needs the hidden input element, which
                // the UITree cannot reference). Adding both would double-fire.
                btns.button("Upload file…")
                    .class("cicfg-btn")
                    .attr("data-cicfg-upload", "1");
                btns.button("Try a broken sample")
                    .class("cicfg-btn")
                    .on_click(Msg::Sample(SAMPLE_DEPLOY_NAME, SAMPLE_DEPLOY_BROKEN));
                btns.button("Clear")
                    .class("cicfg-btn")
                    .on_click(Msg::Clear);
            })
            .class("cicfg-actions");
        })
        .class("cicfg-io");

        c.container(|_| {}).class("cicfg-results");

        pro_block(c);
        upsell(c);
    })
}

/// The pro-only controls (batch upload, custom rules). In the free build
/// this compiles to nothing — the upsell card advertises them instead.
#[cfg(target_arch = "wasm32")]
fn pro_block(c: &mut ContainerBuilder<Msg>) {
    #[cfg(feature = "pro")]
    {
        c.container(|p| {
            p.heading(3, "Pro: custom rules").class("cicfg-pro-heading");
            p.container(|d| {
                d.text("Paste a rules file (see the docs for the format). Rules run on every validated document alongside the built-in schema packs.");
            })
            .class("cicfg-hint");
            p.textarea("").class("cicfg-rules").attr("placeholder", "name: team rules\nrules:\n  - id: no-latest\n    select: \"spec.template.spec.containers[*].image\"\n    condition: not_matches\n    value: \":latest$\"");
        })
        .class("cicfg-panel cicfg-pro-panel");
    }
    #[cfg(not(feature = "pro"))]
    {
        let _ = c;
    }
}

#[cfg(target_arch = "wasm32")]
fn upsell(c: &mut ContainerBuilder<Msg>) {
    #[cfg(not(feature = "pro"))]
    {
        c.container(|u| {
            u.heading(3, "Validate the whole repo, not just one file").class("cicfg-upsell-heading");
            u.list(|items| {
                items.container(|i| { i.text("Batch & directory validation across a whole config tree"); });
                items.container(|i| { i.text("Your own custom rules in a simple, documented format"); });
                items.container(|i| { i.text("CI CLI with proper exit codes — drop it straight into a pipeline"); });
                items.container(|i| { i.text("One-time purchase, team license — the whole team, no subscription"); });
            })
            .class("cicfg-upsell-list");
            u.container(|foot| {
                foot.text("TPT CI Config Validator Pro — $29 one-time, offline, yours to keep.");
            })
            .class("cicfg-upsell-foot");
        })
        .class("cicfg-upsell");
    }
    #[cfg(feature = "pro")]
    {
        let _ = c;
    }
}

// ---------- results ----------

#[cfg(target_arch = "wasm32")]
fn results_view(reports: &Signal<Reports>, has_run: &Signal<bool>) -> UITree<Msg> {
    // One root container — the reconciler requires the view to produce
    // exactly one child (`into_only_child`), so everything nests inside it.
    let reports = reports.get();
    let has_run = has_run.get();
    let mut b = ContainerBuilder::<Msg>::new();
    b.container(|root| {
        if !has_run {
            root.container(|p| {
                p.text("Results appear here after you validate. The validator checks Kubernetes manifests (Deployments, Services, name hygiene), GitHub Actions workflows (job/step shape), Terraform files (brace balance, duplicate resources, hardcoded credentials), and Maven pom.xml structure.");
            })
            .class("cicfg-placeholder");
            return;
        }

        let total_errors: usize = reports.iter().map(Report::error_count).sum();
        let total_warnings: usize = reports.iter().map(Report::warning_count).sum();

        root.container(|summary| {
            if total_errors > 0 {
                summary
                    .container(|s| {
                        s.text(format!("✗ {total_errors} error(s) across {} document(s)", reports.len()));
                    })
                    .class("cicfg-banner cicfg-fail");
            } else if total_warnings > 0 {
                summary
                    .container(|s| { s.text(format!("⚠ Passed with {total_warnings} warning(s)")); })
                    .class("cicfg-banner cicfg-warn");
            } else {
                summary
                    .container(|s| { s.text("✓ All checks passed"); })
                    .class("cicfg-banner cicfg-pass");
            }
            if total_warnings > 0 && total_errors == 0 {
                summary
                    .container(|s| { s.text(format!("{total_warnings} warning(s)")); })
                    .class("cicfg-warn-count");
            }
        })
        .class("cicfg-summary");

        for report in &reports {
            root.container(|card| {
                card.container(|head| {
                    head.container(|n| { n.text(report.document.clone()); }).class("cicfg-docname");
                    head.container(|f| { f.text(report.format.label()); }).class("cicfg-format");
                })
                .class("cicfg-dochead");

                if let Some(err) = &report.parse_error {
                    card.container(|e| { e.text(format!("Parse error: {err}")); })
                        .class("cicfg-parse-error");
                    return;
                }

                if report.findings.is_empty() {
                    card.container(|ok| { ok.text("No issues found."); }).class("cicfg-clean");
                    return;
                }

                for finding in &report.findings {
                    card.container(|row| {
                        row.container(|chip| { chip.text(severity_label(finding.severity)); })
                            .class(match finding.severity {
                                Severity::Error => "cicfg-chip cicfg-chip-error",
                                Severity::Warning => "cicfg-chip cicfg-chip-warning",
                                Severity::Info => "cicfg-chip cicfg-chip-info",
                            });
                        row.container(|body| {
                            body.container(|path| {
                                path.text(if finding.path.is_empty() {
                                    finding.rule_id.clone()
                                } else {
                                    finding.path.clone()
                                });
                            })
                            .class("cicfg-path");
                            body.container(|msg| { msg.text(finding.message.clone()); }).class("cicfg-msg");
                        })
                        .class("cicfg-finding-body");
                    })
                    .class(match finding.severity {
                        Severity::Error => "cicfg-finding cicfg-finding-error",
                        Severity::Warning => "cicfg-finding cicfg-finding-warning",
                        Severity::Info => "cicfg-finding cicfg-finding-info",
                    });
                }
            })
            .class("cicfg-card");
        }
    })
    .class("cicfg-results-view");
    b.into_only_child().expect("results view root")
}

#[cfg(target_arch = "wasm32")]
fn severity_label(s: Severity) -> &'static str {
    match s {
        Severity::Error => "ERROR",
        Severity::Warning => "WARN",
        Severity::Info => "INFO",
    }
}

// ---------- DOM helpers ----------

/// Create-or-update a `<style id>` in the document head.
#[cfg(target_arch = "wasm32")]
fn upsert_style(document: &web_sys::Document, id: &str, css: &str) {
    let head = match document.head() {
        Some(h) => h,
        None => return,
    };
    let style_el = match document.get_element_by_id(id) {
        Some(existing) => existing,
        None => {
            let el = document
                .create_element("style")
                .expect("style element");
            el.set_id(id);
            head.append_child(&el).expect("append style");
            el
        }
    };
    style_el.set_text_content(Some(css));
}

#[cfg(target_arch = "wasm32")]
fn read_textarea(container: &web_sys::Element) -> Option<String> {
    container
        .query_selector(".cicfg-source")
        .ok()?
        .and_then(|el| el.dyn_into::<web_sys::HtmlTextAreaElement>().ok())
        .map(|ta| ta.value())
}

/// The Pro custom-rules panel, parsed. A broken rules file comes back as a
/// leading report (shown like any parse failure) rather than being silently
/// ignored; rules skipped individually are reported the same way.
#[cfg(all(target_arch = "wasm32", feature = "pro"))]
fn custom_rules(container: &web_sys::Element) -> (Option<tpt_config_engine::RuleSet>, Reports) {
    use tpt_config_engine::{parse::{parse_source, Parsed}, rules::rule_set_from_value, Format};

    let text = container
        .query_selector(".cicfg-rules")
        .ok()
        .flatten()
        .and_then(|el| el.dyn_into::<web_sys::HtmlTextAreaElement>().ok())
        .map(|ta| ta.value())
        .unwrap_or_default();
    if text.trim().is_empty() {
        return (None, Vec::new());
    }

    let failure = |message: String| Report {
        document: "<custom rules>".to_string(),
        format: Format::Yaml,
        parse_error: Some(message),
        findings: Vec::new(),
    };
    let doc = match parse_source("custom-rules.yaml", &text) {
        Parsed::Docs(mut docs) if !docs.is_empty() => docs.remove(0),
        Parsed::Docs(_) => return (None, Vec::new()),
        Parsed::Failed(e) => return (None, vec![failure(format!("rules failed to parse: {e}"))]),
    };
    match rule_set_from_value(&doc.root) {
        Ok((set, errors)) => {
            let notes = errors.iter().map(|e| failure(format!("skipped rule: {e}"))).collect();
            (Some(set), notes)
        }
        Err(e) => (None, vec![failure(format!("bad rules: {e}"))]),
    }
}

#[cfg(all(target_arch = "wasm32", not(feature = "pro")))]
fn custom_rules(_container: &web_sys::Element) -> (Option<tpt_config_engine::RuleSet>, Reports) {
    (None, Vec::new())
}

#[cfg(target_arch = "wasm32")]
fn write_textarea(container: &web_sys::Element, text: &str) {
    if let Some(Some(ta)) = container
        .query_selector(".cicfg-source")
        .ok()
        .map(|el| el.and_then(|el| el.dyn_into::<web_sys::HtmlTextAreaElement>().ok()))
    {
        ta.set_value(text);
    }
}

#[cfg(target_arch = "wasm32")]
fn read_filename(container: &web_sys::Element) -> Option<String> {
    container
        .query_selector(".cicfg-filename")
        .ok()?
        .and_then(|el| el.dyn_into::<web_sys::HtmlInputElement>().ok())
        .map(|inp| inp.value())
}

#[cfg(target_arch = "wasm32")]
fn write_filename(container: &web_sys::Element, name: &str) {
    if let Some(Some(inp)) = container
        .query_selector(".cicfg-filename")
        .ok()
        .map(|el| el.and_then(|el| el.dyn_into::<web_sys::HtmlInputElement>().ok()))
    {
        inp.set_value(name);
    }
}

/// Hidden `<input type=file>` + Upload button wiring. Reads the picked file
/// as text and dispatches `FileLoaded`. Pro builds set `multiple` and read
/// every file (batch); free builds take exactly one.
#[cfg(target_arch = "wasm32")]
fn wire_file_input(
    container: &web_sys::Element,
    dispatch: &Rc<dyn Fn(Msg)>,
) -> Result<(), JsValue> {
    let window = web_sys::window().expect("no window");
    let document = window.document().expect("no document");

    let file_input = document
        .create_element("input")?
        .dyn_into::<web_sys::HtmlInputElement>()?;
    file_input.set_type("file");
    file_input.set_accept(".yaml,.yml,.json,.xml,.tf,.hcl,.txt");
    file_input.set_class_name("cicfg-file-input");

    #[cfg(feature = "pro")]
    file_input.set_multiple(true);

    container.append_child(&file_input)?;

    // Upload button: the static tree cannot hold the raw element, so the
    // button's real click handler is attached here, matched by attribute.
    let upload_btn = container
        .query_selector("[data-cicfg-upload]")?
        .ok_or_else(|| JsValue::from_str("missing upload button"))?;
    let input_for_click = file_input.clone();
    let click_handler: Box<dyn FnMut(web_sys::Event)> = Box::new(move |_e| {
        input_for_click.click();
    });
    let handler = wasm_bindgen::closure::Closure::wrap(click_handler);
    upload_btn
        .add_event_listener_with_callback("click", handler.as_ref().unchecked_ref())?;
    handler.forget();

    // File chosen: read as text, then validate. Free builds take one file;
    // pro builds take all picked files in one batch message.
    let dispatch = dispatch.clone();
    let on_change: Box<dyn FnMut(web_sys::Event)> = Box::new(move |e| {
        let input = e
            .target()
            .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok());
        let Some(input) = input else { return };
        let Some(files) = input.files() else { return };

        #[cfg(feature = "pro")]
        read_all_files(&files, &dispatch);

        #[cfg(not(feature = "pro"))]
        if let Some(file) = files.get(0) {
            read_one_file(&file, &dispatch);
        }
    });
    let on_change = wasm_bindgen::closure::Closure::wrap(on_change);
    file_input.add_event_listener_with_callback("change", on_change.as_ref().unchecked_ref())?;
    on_change.forget();

    Ok(())
}

/// Read one picked file and dispatch it (free edition: single file at a time).
#[cfg(all(target_arch = "wasm32", not(feature = "pro")))]
fn read_one_file(file: &web_sys::File, dispatch: &Rc<dyn Fn(Msg)>) {
    let name = file.name();
    let reader = web_sys::FileReader::new().expect("FileReader");
    let reader_for_closure = reader.clone();
    let dispatch = dispatch.clone();
    let onload: Box<dyn FnMut(web_sys::Event)> = Box::new(move |_e| {
        let text = reader_for_closure
            .result()
            .ok()
            .and_then(|v| v.dyn_into::<js_sys::JsString>().ok())
            .and_then(|s| Some(String::from(&s)))
            .unwrap_or_default();
        dispatch(Msg::FileLoaded { name: name.clone(), text });
    });
    let onload = wasm_bindgen::closure::Closure::wrap(onload);
    reader.set_onload(Some(onload.as_ref().unchecked_ref()));
    onload.forget();
    let _ = reader.read_as_text(file);
}

/// Read every picked file, then dispatch one batch message (pro edition).
#[cfg(all(target_arch = "wasm32", feature = "pro"))]
fn read_all_files(files: &web_sys::FileList, dispatch: &Rc<dyn Fn(Msg)>) {
    use std::cell::RefCell;

    let collected: Rc<RefCell<Vec<(String, String)>>> = Rc::new(RefCell::new(Vec::new()));
    let remaining: Rc<RefCell<usize>> = Rc::new(RefCell::new(files.length() as usize));
    let dispatch = dispatch.clone();
    for i in 0..files.length() {
        let Some(file) = files.get(i) else { continue };
        let name = file.name();
        let reader = web_sys::FileReader::new().expect("FileReader");
        let reader_for_closure = reader.clone();
        let collected = collected.clone();
        let remaining = remaining.clone();
        let dispatch = dispatch.clone();
        let onload: Box<dyn FnMut(web_sys::Event)> = Box::new(move |_e| {
            let text = reader_for_closure
                .result()
                .ok()
                .and_then(|v| v.dyn_into::<js_sys::JsString>().ok())
                .and_then(|s| Some(String::from(&s)))
                .unwrap_or_default();
            collected.borrow_mut().push((name.clone(), text));
            *remaining.borrow_mut() -= 1;
            if *remaining.borrow() == 0 {
                dispatch(Msg::BatchLoaded(collected.borrow().clone()));
            }
        });
        let onload = wasm_bindgen::closure::Closure::wrap(onload);
        reader.set_onload(Some(onload.as_ref().unchecked_ref()));
        onload.forget();
        let _ = reader.read_as_text(&file);
    }
}

// ---------- embedded samples ----------

#[cfg(target_arch = "wasm32")]
const SAMPLE_DEPLOY_NAME: &str = "broken-deployment.yaml";

#[cfg(target_arch = "wasm32")]
const SAMPLE_DEPLOY_BROKEN: &str = r#"apiVersion: apps/v1
kind: Deployment
metadata:
  name: Payments_API!!
spec:
  replicas: "three"
  template:
    metadata:
      labels:
        app: payments-api
    spec:
      containers:
        - name: api
          image: registry.internal/payments-api:latest
        - name: sidecar
"#;

// ---------- styles ----------

#[cfg(target_arch = "wasm32")]
const BASE_CSS: &str = r#"
.cicfg-app { --cicfg-error: #c0392b; --cicfg-warn: #b7791f; --cicfg-ok: #1e7d43;
  --cicfg-line: #d5d9de; --cicfg-dim: #5b6470; --cicfg-bg: #f7f8fa; color: #1c2430;
  font: 15px/1.5 system-ui, -apple-system, "Segoe UI", sans-serif; max-width: 980px; margin: 0 auto;
  background: #fff; padding: 22px 20px 26px; border-radius: 12px; }
/* Standalone (trunk) the body is bare; keep the page from looking broken. */
body:has(.cicfg-app) { background: #eef0f3; margin: 0; padding: 18px 10px; }
.cicfg-app * { box-sizing: border-box; }
.cicfg-header { margin-bottom: 18px; }
.cicfg-title { font-size: 26px; margin: 0 0 6px; font-weight: 650; }
.cicfg-sub { color: var(--cicfg-dim); margin: 0; max-width: 72ch; }
.cicfg-io, .cicfg-card, .cicfg-upsell, .cicfg-panel { background: var(--cicfg-bg);
  border: 1px solid var(--cicfg-line); border-radius: 10px; padding: 14px 16px; margin-bottom: 16px; }
.cicfg-filenamerow { display: flex; align-items: center; gap: 10px; margin-bottom: 8px; }
.cicfg-label { font-weight: 600; font-size: 13px; white-space: nowrap; }
.cicfg-filename { flex: 1; padding: 6px 9px; border: 1px solid var(--cicfg-line);
  border-radius: 6px; font: 13px/1.4 ui-monospace, Consolas, monospace; background: #fff; }
.cicfg-source, .cicfg-rules { width: 100%; min-height: 220px; resize: vertical; padding: 10px 12px;
  border: 1px solid var(--cicfg-line); border-radius: 8px; background: #fff;
  font: 13px/1.5 ui-monospace, Consolas, monospace; color: #1c2430; }
.cicfg-rules { min-height: 140px; }
.cicfg-actions { display: flex; flex-wrap: wrap; gap: 8px; margin-top: 10px; }
.cicfg-btn { padding: 8px 14px; border-radius: 7px; border: 1px solid var(--cicfg-line);
  background: #fff; cursor: pointer; font: inherit; font-size: 14px; }
.cicfg-btn:hover { border-color: #9aa4b0; }
.cicfg-primary { background: #14507a; border-color: #14507a; color: #fff; font-weight: 600; }
.cicfg-primary:hover { background: #0f405f; }
.cicfg-file-input { display: none; }
.cicfg-placeholder { color: var(--cicfg-dim); padding: 6px 2px; }
.cicfg-summary { margin-bottom: 12px; }
.cicfg-banner { padding: 10px 14px; border-radius: 8px; font-weight: 650; }
.cicfg-pass { background: #e6f4ea; color: var(--cicfg-ok); border: 1px solid #bfe3cb; }
.cicfg-fail { background: #fdecea; color: var(--cicfg-error); border: 1px solid #f5c6c0; }
.cicfg-warn { background: #fdf6e3; color: var(--cicfg-warn); border: 1px solid #f0e0b0; }
.cicfg-warn-count { color: var(--cicfg-warn); font-size: 13px; margin-top: 4px; }
.cicfg-dochead { display: flex; align-items: baseline; gap: 10px; margin-bottom: 8px; }
.cicfg-docname { font-weight: 650; font-family: ui-monospace, Consolas, monospace; font-size: 14px; }
.cicfg-format { font-size: 12px; color: var(--cicfg-dim); border: 1px solid var(--cicfg-line);
  border-radius: 999px; padding: 1px 9px; background: #fff; }
.cicfg-parse-error { color: var(--cicfg-error); font-family: ui-monospace, Consolas, monospace;
  font-size: 13px; white-space: pre-wrap; }
.cicfg-clean { color: var(--cicfg-ok); }
.cicfg-finding { display: flex; gap: 10px; padding: 8px 10px; border-radius: 7px; margin-bottom: 6px;
  background: #fff; border: 1px solid var(--cicfg-line); }
.cicfg-finding-error { border-left: 3px solid var(--cicfg-error); }
.cicfg-finding-warning { border-left: 3px solid var(--cicfg-warn); }
.cicfg-finding-info { border-left: 3px solid #4a6fa5; }
.cicfg-chip { font-size: 11px; font-weight: 700; letter-spacing: 0.4px; border-radius: 5px;
  padding: 2px 7px; height: fit-content; white-space: nowrap; }
.cicfg-chip-error { background: #fdecea; color: var(--cicfg-error); }
.cicfg-chip-warning { background: #fdf6e3; color: var(--cicfg-warn); }
.cicfg-chip-info { background: #e8eef7; color: #4a6fa5; }
.cicfg-finding-body { min-width: 0; }
.cicfg-path { font-family: ui-monospace, Consolas, monospace; font-size: 13px; color: #14507a;
  word-break: break-all; }
.cicfg-msg { font-size: 14px; margin-top: 1px; }
.cicfg-upsell { border-style: dashed; }
.cicfg-upsell-heading { margin: 0 0 8px; font-size: 16px; }
.cicfg-upsell-list { margin: 0; padding-left: 20px; }
.cicfg-upsell-list > * { margin: 3px 0; }
.cicfg-upsell-foot { margin-top: 10px; color: var(--cicfg-dim); font-size: 13px; }
.cicfg-pro-heading { margin: 0 0 6px; font-size: 15px; }
.cicfg-footer { margin-top: 4px; padding-top: 12px; border-top: 1px solid var(--cicfg-line);
  color: var(--cicfg-dim); font-size: 13px; text-align: center; }
.cicfg-footer a { color: #14507a; }
.cicfg-hint { color: var(--cicfg-dim); font-size: 13px; margin-bottom: 8px; }
"#;
