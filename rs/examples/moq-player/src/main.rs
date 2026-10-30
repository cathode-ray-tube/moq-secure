use std::{cell::RefCell, process, rc::Rc, time::Duration};

use gtk4 as gtk;
use gtk::{glib, prelude::*};

use gstreamer as gst;
use gst::prelude::*;

const DEFAULT_URL: &str = "https://cdn.moq.dev/demo";
const DEFAULT_BROADCAST: &str = "bbb.hang";
const AUDIO_TS_OFFSET_NS: i64 = -70_000_000;

struct Player {
    pipeline: gst::Pipeline,
    video_sink: gst::Element,
}

impl Player {
    fn new(url: &str, broadcast: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let pipeline = gst::Pipeline::new();

        let source = gst::ElementFactory::make("moqsrc")
            .property("url", url)
            .property("broadcast", broadcast)
            .build()?;

        let video_parser = gst::ElementFactory::make("h264parse")
            .property("config-interval", 1i32)
            .build()?;

        let video_queue = gst::ElementFactory::make("queue")
            .property("max-size-time", 500_000_000u64)
            .property("max-size-buffers", 0u32)
            .property("max-size-bytes", 0u32)
            .build()?;

        let video_decoder = gst::ElementFactory::make("decodebin3").build()?;
        let decoded_video_queue = gst::ElementFactory::make("queue")
            .property("max-size-time", 250_000_000u64)
            .property("max-size-buffers", 0u32)
            .property("max-size-bytes", 0u32)
            .build()?;
        let video_convert = gst::ElementFactory::make("videoconvert").build()?;

        let video_sink = gst::ElementFactory::make("gtk4paintablesink")
            .property("sync", true)
            .property("async", true)
            .build()
            .map_err(|e| format!("Could not create gtk4paintablesink: {e}"))?;

        let audio_parser = gst::ElementFactory::make("aacparse").build()?;
        let audio_decoder = gst::ElementFactory::make("decodebin3").build()?;
        let audio_queue = gst::ElementFactory::make("queue")
            .property("max-size-time", 500_000_000u64)
            .property("max-size-buffers", 0u32)
            .property("max-size-bytes", 0u32)
            .build()?;
        let audio_convert = gst::ElementFactory::make("audioconvert").build()?;
        let audio_resample = gst::ElementFactory::make("audioresample")
            .property("quality", 10i32)
            .build()?;

        let audio_caps = gst::ElementFactory::make("capsfilter")
            .property(
                "caps",
                gst::Caps::builder("audio/x-raw")
                    .field("format", "S16LE")
                    .field("layout", "interleaved")
                    .field("rate", 48_000i32)
                    .field("channels", 2i32)
                    .build(),
            )
            .build()?;

        let audio_volume = gst::ElementFactory::make("volume")
            .property("volume", 0.7f64)
            .build()?;

        let audio_sink = gst::ElementFactory::make("pipewiresink")
            .property("sync", true)
            .property("async", true)
            .property("ts-offset", AUDIO_TS_OFFSET_NS)
            .build()
            .map_err(|e| format!("Could not create pipewiresink: {e}"))?;

        pipeline.add_many([
            &source,
            &video_parser,
            &video_queue,
            &video_decoder,
            &decoded_video_queue,
            &video_convert,
            &video_sink,
            &audio_parser,
            &audio_decoder,
            &audio_queue,
            &audio_convert,
            &audio_resample,
            &audio_caps,
            &audio_volume,
            &audio_sink,
        ])?;

        gst::Element::link_many([&video_parser, &video_queue, &video_decoder])?;
        gst::Element::link_many([
            &decoded_video_queue,
            &video_convert,
            &video_sink,
        ])?;
        gst::Element::link_many([&audio_parser, &audio_decoder])?;
        gst::Element::link_many([
            &audio_queue,
            &audio_convert,
            &audio_resample,
            &audio_caps,
            &audio_volume,
            &audio_sink,
        ])?;

        video_queue.set_property_from_str("leaky", "no");
        decoded_video_queue.set_property_from_str("leaky", "no");
        audio_queue.set_property_from_str("leaky", "no");

        let video_parser_weak = video_parser.downgrade();
        let audio_parser_weak = audio_parser.downgrade();

        source.connect_pad_added(move |_source, pad| {
            let caps = pad
                .current_caps()
                .unwrap_or_else(|| pad.query_caps(None));

            let Some(structure) = caps.structure(0) else {
                eprintln!("MoQ pad has no caps: {caps}");
                return;
            };

            let media_type = structure.name();
            let target = if media_type.starts_with("video/") {
                video_parser_weak.upgrade()
            } else if media_type.starts_with("audio/") {
                audio_parser_weak.upgrade()
            } else {
                eprintln!("Ignoring unsupported MoQ pad: {caps}");
                return;
            };

            let Some(target) = target else {
                return;
            };
            let Some(sink_pad) = target.static_pad("sink") else {
                eprintln!("Target has no sink pad for {media_type}");
                return;
            };

            if sink_pad.is_linked() {
                eprintln!("Already linked a {media_type} pad; ignoring extra pad");
                return;
            }

            if let Err(error) = pad.link(&sink_pad) {
                eprintln!("Could not link {media_type} pad: {error}");
            }
        });

        let decoded_video_queue_weak = decoded_video_queue.downgrade();
        video_decoder.connect_pad_added(move |_decoder, pad| {
            let caps = pad
                .current_caps()
                .unwrap_or_else(|| pad.query_caps(None));

            let Some(structure) = caps.structure(0) else {
                return;
            };
            if !structure.name().starts_with("video/") {
                return;
            }

            let Some(queue) = decoded_video_queue_weak.upgrade() else {
                return;
            };
            let Some(sink_pad) = queue.static_pad("sink") else {
                return;
            };

            if !sink_pad.is_linked() {
                if let Err(error) = pad.link(&sink_pad) {
                    eprintln!("Could not link decoded video: {error}");
                }
            }
        });

        let audio_queue_weak = audio_queue.downgrade();
        audio_decoder.connect_pad_added(move |_decoder, pad| {
            let caps = pad
                .current_caps()
                .unwrap_or_else(|| pad.query_caps(None));

            let Some(structure) = caps.structure(0) else {
                return;
            };
            if !structure.name().starts_with("audio/") {
                return;
            }

            let Some(queue) = audio_queue_weak.upgrade() else {
                return;
            };
            let Some(sink_pad) = queue.static_pad("sink") else {
                return;
            };

            if !sink_pad.is_linked() {
                if let Err(error) = pad.link(&sink_pad) {
                    eprintln!("Could not link decoded audio: {error}");
                }
            }
        });

        Ok(Self {
            pipeline,
            video_sink,
        })
    }

    fn play(&self) -> Result<(), gst::StateChangeError> {
        self.pipeline.set_state(gst::State::Playing)?;
        Ok(())
    }

    fn stop(&self) {
        if let Err(error) = self.pipeline.set_state(gst::State::Null) {
            eprintln!("Could not stop playback: {error}");
        }
    }

    fn paintable(&self) -> Option<gtk::gdk::Paintable> {
        self.video_sink
            .property::<Option<gtk::gdk::Paintable>>("paintable")
    }

    fn install_bus_watch(&self, status: gtk::Label) {
        let Some(bus) = self.pipeline.bus() else {
            eprintln!("Pipeline has no bus");
            return;
        };

        bus.add_watch_local(move |_bus, message| {
            use gst::MessageView;

            match message.view() {
                MessageView::Error(error) => {
                    let source = error
                        .src()
                        .map(|src| src.path_string().to_string())
                        .unwrap_or_else(|| "<unknown>".to_string());

                    eprintln!("GStreamer error from {source}: {}", error.error());
                    if let Some(debug) = error.debug() {
                        eprintln!("Debug: {debug}");
                    }

                    status.set_text("Playback error");
                }
                MessageView::Warning(warning) => {
                    eprintln!("GStreamer warning: {}", warning.error());
                    if let Some(debug) = warning.debug() {
                        eprintln!("Debug: {debug}");
                    }
                }
                MessageView::Eos(..) => status.set_text("Stream ended"),
                _ => {}
            }

            glib::ControlFlow::Continue
        })
        .expect("Could not install GStreamer bus watch");
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.stop();
    }
}

fn install_css() {
    let provider = gtk::CssProvider::new();
    provider.load_from_data(
        r#"
        window {
            background-color: #080b12;
            color: #f4f6fb;
            font-family: Arial, Helvetica, sans-serif;
        }

        .page {
            background-image: radial-gradient(ellipse at 15% 0%, #172544, #080b12 65%);
        }

        .brand-mark {
            background-image: linear-gradient(135deg, #66e3c4, #538bff);
            color: #071018;
            border-radius: 18px;
            font-size: 30px;
            font-weight: 800;
            min-width: 58px;
            min-height: 58px;
        }

        .title {
            color: #f4f6fb;
            font-size: 42px;
            font-weight: 800;
        }

        .subtitle {
            color: #9daac1;
            font-size: 18px;
        }

        .card {
            background-color: #101624;
            border: 1px solid #273248;
            border-radius: 22px;
        }

        .player-screen {
            background-image: radial-gradient(ellipse at center, #1c2b43, #070a10);
            border-radius: 14px;
        }

        .placeholder-title {
            color: #b6c2d5;
            font-size: 22px;
            font-weight: 700;
        }

        .placeholder-copy {
            color: #8d9bb1;
            font-size: 16px;
        }

        .play-symbol {
            color: #66e3c4;
            font-size: 54px;
        }

        .footer {
            color: #aab5c8;
            font-size: 14px;
        }

        .form-title {
            color: #f4f6fb;
            font-size: 23px;
            font-weight: 700;
        }

        .field-label {
            color: #b8c3d6;
            font-size: 15px;
        }

        entry {
            min-height: 48px;
            padding: 0 12px;
            border: 1px solid #35435a;
            border-radius: 11px;
            background-color: #090e18;
            color: #ffffff;
            font-size: 16px;
        }

        entry:focus {
            border-color: #66e3c4;
            box-shadow: 0 0 0 3px alpha(#66e3c4, 0.35);
        }

        button.connect {
            min-height: 54px;
            border: 0;
            border-radius: 12px;
            background-image: linear-gradient(100deg, #66e3c4, #69a7ff);
            color: #071018;
            font-size: 18px;
            font-weight: 800;
        }

        button.connect:hover {
            background-image: linear-gradient(100deg, #7aefd2, #80b5ff);
        }

        button.connect:active {
            background-image: linear-gradient(100deg, #56cdb0, #5b94e5);
        }

        .hint {
            color: #8391a8;
            font-size: 13px;
        }

        separator {
            background-color: #273248;
        }
        "#,
    );

    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

fn build_ui(app: &gtk::Application) {
    install_css();

    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .title("MoQ TV")
        .default_width(1280)
        .default_height(800)
        .build();

    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.add_css_class("page");
    root.set_margin_start(28);
    root.set_margin_end(28);
    root.set_margin_top(24);
    root.set_margin_bottom(24);

    let header = gtk::Box::new(gtk::Orientation::Horizontal, 18);
    header.set_margin_bottom(24);

    let mark = gtk::Label::new(Some("M"));
    mark.add_css_class("brand-mark");
    header.append(&mark);

    let title_box = gtk::Box::new(gtk::Orientation::Vertical, 2);
    let title = gtk::Label::new(Some("MoQ TV"));
    title.set_halign(gtk::Align::Start);
    title.add_css_class("title");

    let subtitle = gtk::Label::new(Some("Low-latency live video"));
    subtitle.set_halign(gtk::Align::Start);
    subtitle.add_css_class("subtitle");

    title_box.append(&title);
    title_box.append(&subtitle);
    header.append(&title_box);
    root.append(&header);

    let content = gtk::Box::new(gtk::Orientation::Horizontal, 22);
    content.set_vexpand(true);

    let player_card = gtk::Box::new(gtk::Orientation::Vertical, 0);
    player_card.add_css_class("card");
    player_card.set_hexpand(true);
    player_card.set_vexpand(true);
    player_card.set_margin_end(0);
    player_card.set_margin_top(0);
    player_card.set_margin_bottom(0);
    player_card.set_margin_start(0);
    player_card.set_spacing(0);
    player_card.set_overflow(gtk::Overflow::Hidden);

    let screen_frame = gtk::Frame::new(None);
    screen_frame.add_css_class("player-screen");
    screen_frame.set_margin_start(14);
    screen_frame.set_margin_end(14);
    screen_frame.set_margin_top(14);
    screen_frame.set_margin_bottom(0);
    screen_frame.set_hexpand(true);
    screen_frame.set_vexpand(true);

    let overlay = gtk::Overlay::new();
    overlay.set_hexpand(true);
    overlay.set_vexpand(true);

    let picture = gtk::Picture::builder()
        .hexpand(true)
        .vexpand(true)
        .can_shrink(true)
        .keep_aspect_ratio(true)
        .build();
    overlay.set_child(Some(&picture));

    let placeholder = gtk::Box::new(gtk::Orientation::Vertical, 8);
    placeholder.set_halign(gtk::Align::Center);
    placeholder.set_valign(gtk::Align::Center);
    placeholder.set_halign(gtk::Align::Center);

    let play_symbol = gtk::Label::new(Some("▶"));
    play_symbol.add_css_class("play-symbol");
    let placeholder_title = gtk::Label::new(Some("Ready to watch"));
    placeholder_title.add_css_class("placeholder-title");
    let placeholder_copy = gtk::Label::new(Some("Connect to start the broadcast"));
    placeholder_copy.add_css_class("placeholder-copy");

    placeholder.append(&play_symbol);
    placeholder.append(&placeholder_title);
    placeholder.append(&placeholder_copy);
    overlay.add_overlay(&placeholder);
    screen_frame.set_child(Some(&overlay));

    let footer = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    footer.set_margin_start(18);
    footer.set_margin_end(18);
    footer.set_margin_top(12);
    footer.set_margin_bottom(14);
    footer.set_hexpand(true);

    let status = gtk::Label::new(Some("Not connected"));
    status.set_halign(gtk::Align::Start);
    status.set_hexpand(true);
    status.add_css_class("footer");

    let technology = gtk::Label::new(Some("MoQ · WebTransport"));
    technology.set_halign(gtk::Align::End);
    technology.add_css_class("footer");

    footer.append(&status);
    footer.append(&technology);
    player_card.append(&screen_frame);
    player_card.append(&footer);

    let panel = gtk::Box::new(gtk::Orientation::Vertical, 8);
    panel.add_css_class("card");
    panel.set_width_request(340);
    panel.set_margin_top(0);
    panel.set_margin_bottom(0);
    panel.set_margin_start(0);
    panel.set_margin_end(0);

    let form_content = gtk::Box::new(gtk::Orientation::Vertical, 4);
    form_content.set_margin_start(22);
    form_content.set_margin_end(22);
    form_content.set_margin_top(22);
    form_content.set_margin_bottom(22);

    let form_title = gtk::Label::new(Some("Stream connection"));
    form_title.set_halign(gtk::Align::Start);
    form_title.set_margin_bottom(10);
    form_title.add_css_class("form-title");
    form_content.append(&form_title);

    let url_label = gtk::Label::new(Some("Relay URL"));
    url_label.set_halign(gtk::Align::Start);
    url_label.set_margin_top(8);
    url_label.add_css_class("field-label");

    let url_entry = gtk::Entry::builder()
        .text(DEFAULT_URL)
        .placeholder_text("https://relay.example")
        .build();
    url_entry.set_activates_default(true);

    let port_label = gtk::Label::new(Some("Port (optional)"));
    port_label.set_halign(gtk::Align::Start);
    port_label.set_margin_top(8);
    port_label.add_css_class("field-label");

    let port_entry = gtk::Entry::builder()
        .placeholder_text("Use URL default")
        .input_purpose(gtk::InputPurpose::Digits)
        .build();
    port_entry.set_activates_default(true);

    let broadcast_label = gtk::Label::new(Some("Broadcast name"));
    broadcast_label.set_halign(gtk::Align::Start);
    broadcast_label.set_margin_top(8);
    broadcast_label.add_css_class("field-label");

    let broadcast_entry = gtk::Entry::builder()
        .text(DEFAULT_BROADCAST)
        .build();
    broadcast_entry.set_activates_default(true);

    let connect_button = gtk::Button::with_label("Connect & play");
    connect_button.add_css_class("connect");
    connect_button.set_margin_top(16);
    connect_button.set_receives_default(true);

    let hint = gtk::Label::new(Some(
        "Leave the port blank to use the port specified by the URL.",
    ));
    hint.set_halign(gtk::Align::Start);
    hint.set_wrap(true);
    hint.set_margin_top(6);
    hint.add_css_class("hint");

    form_content.append(&url_label);
    form_content.append(&url_entry);
    form_content.append(&port_label);
    form_content.append(&port_entry);
    form_content.append(&broadcast_label);
    form_content.append(&broadcast_entry);
    form_content.append(&connect_button);
    form_content.append(&hint);
    panel.append(&form_content);

    content.append(&player_card);
    content.append(&panel);
    root.append(&content);

    window.set_child(Some(&root));
    window.set_default_widget(Some(&connect_button));

    let current_player: Rc<RefCell<Option<Rc<Player>>>> = Rc::new(RefCell::new(None));

    {
        let current_player = Rc::clone(&current_player);
        let url_entry = url_entry.clone();
        let port_entry = port_entry.clone();
        let broadcast_entry = broadcast_entry.clone();
        let status = status.clone();
        let picture = picture.clone();
        let placeholder = placeholder.clone();

        connect_button.connect_clicked(move |_| {
            let raw_url = url_entry.text().trim().to_string();
            let port = port_entry.text().trim().to_string();
            let broadcast = broadcast_entry.text().trim().to_string();

            if raw_url.is_empty() || broadcast.is_empty() {
                status.set_text("Enter a relay URL and broadcast name");
                return;
            }

            let mut relay_url = match url::Url::parse(&raw_url) {
                Ok(url) => url,
                Err(error) => {
                    status.set_text(&format!("Invalid URL: {error}"));
                    return;
                }
            };

            if relay_url.scheme() != "https" {
                status.set_text("Relay URL must use HTTPS");
                return;
            }

            if !port.is_empty() {
                let port_number: u16 = match port.parse() {
                    Ok(port) if port > 0 => port,
                    _ => {
                        status.set_text("Enter a valid port number (1–65535)");
                        return;
                    }
                };

                if let Err(error) = relay_url.set_port(Some(port_number)) {
                    status.set_text(&format!("Could not set port: {error:?}"));
                    return;
                }
            }

            status.set_text("Connecting…");
            picture.set_paintable(None);
            placeholder.set_visible(true);

            let new_player = match Player::new(relay_url.as_str(), &broadcast) {
                Ok(player) => Rc::new(player),
                Err(error) => {
                    status.set_text(&format!("Could not create player: {error}"));
                    return;
                }
            };

            new_player.install_bus_watch(status.clone());

            if let Some(old_player) = current_player.borrow_mut().take() {
                old_player.stop();
            }

            *current_player.borrow_mut() = Some(Rc::clone(&new_player));

            if let Err(error) = new_player.play() {
                status.set_text(&format!("Could not start playback: {error}"));
                return;
            }

            let player_for_timeout = Rc::clone(&new_player);
            let picture_for_timeout = picture.clone();
            let placeholder_for_timeout = placeholder.clone();
            let status_for_timeout = status.clone();

            glib::timeout_add_local(Duration::from_millis(250), move || {
                let Some(paintable) = player_for_timeout.paintable() else {
                    return glib::ControlFlow::Continue;
                };

                if paintable.intrinsic_width() > 0 && paintable.intrinsic_height() > 0 {
                    picture_for_timeout.set_paintable(Some(&paintable));
                    placeholder_for_timeout.set_visible(false);
                    status_for_timeout.set_text("Connected");
                    glib::ControlFlow::Break
                } else {
                    glib::ControlFlow::Continue
                }
            });
        });
    }

    {
        let app = app.clone();
        window.connect_close_request(move |_| {
            app.quit();
            glib::Propagation::Proceed
        });
    }

    window.present();
}

fn main() {
    if let Err(error) = gst::init() {
        eprintln!("Could not initialize GStreamer: {error}");
        process::exit(1);
    }

    let app = gtk::Application::builder()
        .application_id("com.cathode-ray-tube.moq-player")
        .build();

    app.connect_activate(build_ui);
    app.run();
}

