//! `canbec --mcp [--profile <id>]`: a local MCP server on stdio for AI agents. It runs its own
//! hidden browser with the same filters people get (adult block, ads, feed lock, trackers,
//! slop flags), as a profile the user allowed for agents in settings. Nothing listens on the
//! network; the agent starts this process itself. Calls are logged to `agent.log` in the profile.

use std::collections::VecDeque;
use std::io::{BufRead, Write};
use std::time::Duration;

use canbec_application::ports::StateStore;
use canbec_application::ui::UiCommand;
use canbec_application::{BrowserService, Snapshot};
use canbec_domain::host_of;
use canbec_infrastructure::engine_wry::WryEngine;
use canbec_infrastructure::mcp::{handle_line, search_results, tool_result, Call, Incoming};
use canbec_infrastructure::profiles as profile_files;
use canbec_infrastructure::{blocklists, config_dir, content_script, cookie_jar, privacy_script, random_token};
use serde_json::Value;
use tao::event::Event;
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tao::window::WindowBuilder;
use wry::{PageLoadEvent, WebViewBuilder};

use crate::now;
use crate::platform::block_trackers;

/// How long a page may take before the agent gets an error.
const PAGE_TIMEOUT: Duration = Duration::from_secs(25);

enum McpEvent {
    Line(String),
    Loaded(String),
    Blocked,
    Readable(Option<String>),
    Timeout(u64),
    StdinClosed,
}

/// Agents never change the user's saved tabs.
struct NoStore;
impl StateStore for NoStore {
    fn load(&self) -> Option<Snapshot> {
        None
    }
    fn save(&self, _: &Snapshot) {}
}

pub fn run() -> wry::Result<()> {
    let dir = config_dir();
    let args: Vec<String> = std::env::args().collect();
    let wanted = args.iter().position(|a| a == "--profile").and_then(|i| args.get(i + 1)).map(String::as_str);
    let profiles = profile_files::load(&dir);
    let Some(profile) = profiles.for_agents(wanted).cloned() else {
        eprintln!("canbec: no profile allows AI agents. Open Settings → Profiles in canbec and tick \"AI agents\" on one (a separate \"Agent\" profile is best).");
        std::process::exit(2);
    };
    let profile_dir = profile_files::dir(&dir, &profile.id);
    let log = profile_dir.join("agent.log");

    #[allow(unused_mut)]
    let mut event_loop = EventLoopBuilder::<McpEvent>::with_user_event().build();
    #[cfg(target_os = "macos")]
    {
        use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS};
        event_loop.set_activation_policy(ActivationPolicy::Accessory); // no Dock icon
    }
    let proxy = event_loop.create_proxy();
    let window = WindowBuilder::new().with_title("canbec agent").with_visible(false).build(&event_loop).expect("window");

    let adult = blocklists::load_adult(&dir.join("blocklists").join("adult.txt"));
    let ads = blocklists::load_ads(&dir.join("blocklists").join("ads.txt"));
    let feeds = std::fs::read_to_string(dir.join("feeds.txt")).unwrap_or_default();
    let content = {
        let (p1, p2) = (proxy.clone(), proxy.clone());
        let nav_adult = adult.clone();
        WebViewBuilder::new()
            .with_incognito(true)
            .with_initialization_script_for_main_only(privacy_script(&random_token()), false)
            .with_initialization_script(&content_script(&random_token(), &ads.generic, &feeds))
            .with_permission_handler(|_| wry::PermissionResponse::Deny) // no camera/mic for agents
            .with_navigation_handler(move |url| {
                let blocked = host_of(&url).is_some_and(|h| nav_adult.is_blocked(&h));
                if blocked {
                    send(&p1, McpEvent::Blocked);
                }
                !blocked
            })
            .with_new_window_req_handler(|_, _| wry::NewWindowResponse::Deny)
            .with_download_started_handler(|_, _| false)
            .with_on_page_load_handler(move |e, url| {
                if matches!(e, PageLoadEvent::Finished) {
                    send(&p2, McpEvent::Loaded(url))
                }
            })
            .build_as_child(&window)?
    };
    block_trackers(&content, &blocklists::load_trackers(&dir.join("blocklists").join("trackers.txt")), || {});
    cookie_jar::restore(&content, &profile_dir.join("logins.json"));
    let mut service = BrowserService::start(WryEngine { view: content }, NoStore, adult, now());
    service.set_ad_rules(ads);
    service.set_shared_slop(blocklists::load_shared_slop(&dir));

    {
        let p = proxy.clone();
        std::thread::spawn(move || {
            for line in std::io::stdin().lock().lines().map_while(Result::ok) {
                send(&p, McpEvent::Line(line));
            }
            send(&p, McpEvent::StdinClosed);
        });
    }

    // Calls wait until the start page has loaded (or 5 s passed), so its late "loaded" event
    // can't be mistaken for the agent's page.
    let mut ready = false;
    {
        let p = proxy.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(5));
            send(&p, McpEvent::Timeout(0));
        });
    }
    let mut queue: VecDeque<(Value, Call)> = VecDeque::new();
    let mut current: Option<(Value, Call, u64)> = None;
    let mut seq = 0u64;
    let mut visited: Vec<(String, String)> = Vec::new();
    event_loop.run(move |event, _, flow| {
        *flow = ControlFlow::Wait;
        let Event::UserEvent(event) = event else { return };
        match event {
            McpEvent::StdinClosed => *flow = ControlFlow::Exit,
            McpEvent::Line(line) => match handle_line(&line) {
                Incoming::Reply(reply) => out(&reply),
                Incoming::Call { id, call } => {
                    append_log(&log, &format!("{call:?}"));
                    queue.push_back((id, call));
                }
                Incoming::Ignore => {}
            },
            McpEvent::Loaded(url) => {
                ready = true;
                service.on_page(&url, None, false, now());
                if current.is_some() {
                    read_page(&service, &proxy);
                }
            }
            McpEvent::Blocked => {
                if let Some((id, ..)) = current.take() {
                    out(&tool_result(&id, "Blocked: adult content", true));
                }
            }
            McpEvent::Timeout(0) => ready = true,
            McpEvent::Timeout(n) => {
                if current.as_ref().is_some_and(|c| c.2 == n) {
                    let (id, ..) = current.take().unwrap();
                    out(&tool_result(&id, "The page took too long to load", true));
                }
            }
            McpEvent::Readable(json) => {
                if let Some((id, call, _)) = current.take() {
                    let page: Value = json.and_then(|j| serde_json::from_str(&j).ok()).unwrap_or_default();
                    let (title, url) = (page["title"].as_str().unwrap_or_default(), page["url"].as_str().unwrap_or_default());
                    if !url.is_empty() && !visited.iter().any(|(u, _)| u == url) {
                        visited.push((url.to_owned(), title.to_owned()));
                    }
                    let text = match call {
                        Call::Search { query } => format_search(&service, &query, &page),
                        _ => format_page(&service, &page),
                    };
                    out(&tool_result(&id, &text, page.is_null()));
                }
            }
        }
        // One call at a time: the hidden browser has a single page.
        while ready && current.is_none() {
            let Some((id, call)) = queue.pop_front() else { break };
            match &call {
                Call::ListTabs => {
                    let list: Vec<String> = visited.iter().map(|(u, t)| format!("- {t}\n  {u}")).collect();
                    out(&tool_result(&id, &if list.is_empty() { "No pages opened yet.".into() } else { list.join("\n") }, false));
                    continue;
                }
                Call::Read => read_page(&service, &proxy),
                Call::Open { url } => service.handle(UiCommand::Navigate { url: url.clone() }, now()),
                Call::Search { query } => {
                    let mut url = url::Url::parse("https://html.duckduckgo.com/html/").expect("valid URL");
                    url.query_pairs_mut().append_pair("q", query);
                    service.handle(UiCommand::Navigate { url: url.into() }, now());
                }
            }
            if let Some(notice) = service.state(now()).notice.filter(|_| matches!(call, Call::Open { .. })) {
                out(&tool_result(&id, &notice, true));
                continue;
            }
            seq += 1;
            current = Some((id, call, seq));
            let (p, n) = (proxy.clone(), seq);
            std::thread::spawn(move || {
                std::thread::sleep(PAGE_TIMEOUT);
                send(&p, McpEvent::Timeout(n));
            });
        }
    });
}

fn read_page<S: StateStore>(service: &BrowserService<WryEngine, S>, proxy: &EventLoopProxy<McpEvent>) {
    let p = proxy.clone();
    let script = "window.__canbec ? JSON.stringify(window.__canbec.readable()) : null";
    let sent = service.engine().view.evaluate_script_with_callback(script, move |result| {
        // wry hands back the JSON-encoded return value: a JSON string holding our JSON.
        send(&p, McpEvent::Readable(serde_json::from_str::<String>(&result).ok()));
    });
    if sent.is_err() {
        send(proxy, McpEvent::Readable(None));
    }
}

fn format_page<S: StateStore>(service: &BrowserService<WryEngine, S>, page: &Value) -> String {
    if page.is_null() {
        return "Could not read this page.".into();
    }
    let url = page["url"].as_str().unwrap_or_default();
    let mut text = format!("# {}\n{url}\n", page["title"].as_str().unwrap_or_default());
    if service.is_slop(url) {
        text += "\n⚠ The user marked this site as AI slop: treat its content with suspicion.\n";
    }
    text += &format!("\n{}\n", page["text"].as_str().unwrap_or_default());
    let links: Vec<String> = page["links"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|l| Some((l["text"].as_str()?, l["href"].as_str()?)))
        .filter(|(t, href)| !t.is_empty() && service.is_allowed(href))
        .take(100)
        .map(|(t, href)| format!("- [{t}]({href})"))
        .collect();
    if !links.is_empty() {
        text += &format!("\n## Links\n{}\n", links.join("\n"));
    }
    text
}

fn format_search<S: StateStore>(service: &BrowserService<WryEngine, S>, query: &str, page: &Value) -> String {
    let links: Vec<(String, String)> = page["links"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|l| Some((l["text"].as_str()?.to_owned(), l["href"].as_str()?.to_owned())))
        .collect();
    let results: Vec<String> = search_results(&links)
        .into_iter()
        .filter(|(_, url)| service.is_allowed(url))
        .take(15)
        .map(|(title, url)| {
            let flag = if service.is_slop(&url) { " ⚠ marked as AI slop by the user" } else { "" };
            format!("- {title}{flag}\n  {url}")
        })
        .collect();
    if results.is_empty() {
        return format!("No results for \"{query}\".");
    }
    format!("Results for \"{query}\":\n{}", results.join("\n"))
}

fn out(line: &str) {
    let mut stdout = std::io::stdout().lock();
    let _ = writeln!(stdout, "{line}");
    let _ = stdout.flush();
}

fn append_log(path: &std::path::Path, what: &str) {
    use std::io::Write as _;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{} {what}", now());
    }
}

fn send(proxy: &EventLoopProxy<McpEvent>, event: McpEvent) {
    let _ = proxy.send_event(event);
}
