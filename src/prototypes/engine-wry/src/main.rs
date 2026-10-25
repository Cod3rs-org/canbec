//! Engine bake-off: wry (system webview: WebKit / WebView2 / WebKitGTK).
//! Usage: engine-wry [url]   — opens a 1280x800 window at (100,100) you can click through.

use tao::{
    dpi::{LogicalPosition, LogicalSize},
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use wry::WebViewBuilder;

fn main() -> wry::Result<()> {
    let url = std::env::args().nth(1).unwrap_or("http://localhost:5080".into());
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title(format!("engine-wry — {url}"))
        .with_inner_size(LogicalSize::new(1280.0, 800.0))
        .with_position(LogicalPosition::new(100.0, 100.0))
        .build(&event_loop)
        .unwrap();

    #[cfg(not(target_os = "linux"))]
    let builder = WebViewBuilder::new().with_url(&url).build(&window);
    // ponytail: Linux needs a GTK container; tao's helper provides it.
    #[cfg(target_os = "linux")]
    let builder = {
        use tao::platform::unix::WindowExtUnix;
        use wry::WebViewBuilderExtUnix;
        WebViewBuilder::new().with_url(&url).build_gtk(window.default_vbox().unwrap())
    };
    let _webview = builder?;

    event_loop.run(move |event, _, flow| {
        *flow = ControlFlow::Wait;
        if let Event::WindowEvent { event: WindowEvent::CloseRequested, .. } = event {
            *flow = ControlFlow::Exit;
        }
    });
}
