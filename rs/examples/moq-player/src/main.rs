use std::{cell::Cell, process, rc::Rc, time::Duration};

use gtk4 as gtk;
use gtk::{glib, prelude::*};

use gstreamer as gst;
use gst::prelude::*;

const MOQ_URL: &str = "https://cdn.moq.dev/demo";
const MOQ_BROADCAST: &str = "bbb.hang";
const AUDIO_TS_OFFSET_NS: i64 = -70_000_000;

struct Player {
    pipeline: gst::Pipeline,
    video_sink: gst::Element,
}

impl Player {
    fn new(
        url: &str,
        broadcast: &str,
        enable_video: bool,
        enable_audio: bool,
    ) -> Result<Self, Box<dyn std::error::Error>> {
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
        video_queue.set_property_from_str("leaky", "no");

        let video_decoder = gst::ElementFactory::make("decodebin3").build()?;

        let decoded_video_queue = gst::ElementFactory::make("queue")
            .property("max-size-time", 250_000_000u64)
            .property("max-size-buffers", 0u32)
            .property("max-size-bytes", 0u32)
            .build()?;
        decoded_video_queue.set_property_from_str("leaky", "no");

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
        audio_queue.set_property_from_str("leaky", "no");

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

        let video_parser_weak = video_parser.downgrade();
        let audio_parser_weak = audio_parser.downgrade();

        source.connect_pad_added(move |_source, pad| {
            let name = pad.name().to_string();
            let caps = pad
                .current_caps()
                .unwrap_or_else(|| pad.query_caps(None));

            let Some(structure) = caps.structure(0) else {
                eprintln!("MoQ pad {name} has no caps: {caps}");
                return;
            };

            let media_type = structure.name();
            let is_video = media_type.starts_with("video/");
            let is_audio = media_type.starts_with("audio/");

            println!("MoQ pad {name} caps: {caps}");

            let target = if is_video && enable_video {
                video_parser_weak.upgrade()
            } else if is_audio && enable_audio {
                audio_parser_weak.upgrade()
            } else {
                if (is_video && !enable_video) || (is_audio && !enable_audio) {
                    println!("Ignoring disabled MoQ pad {name}");
                } else {
                    println!("Ignoring unsupported MoQ pad {name}: {caps}");
                }
                return;
            };

            let Some(target) = target else {
                return;
            };

            let Some(target_sink) = target.static_pad("sink") else {
                eprintln!("Target has no sink pad for {media_type}");
                return;
            };

            if target_sink.is_linked() {
                eprintln!(
                    "Already linked a {media_type} pad; ignoring additional pad {name}"
                );
                return;
            }

            match pad.link(&target_sink) {
                Ok(_) => println!("Linked MoQ pad {name}"),
                Err(error) => eprintln!("Could not link MoQ pad {name}: {error}"),
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

            if sink_pad.is_linked() {
                return;
            }

            if let Err(error) = pad.link(&sink_pad) {
                eprintln!("Could not link decoded video: {error}");
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

            if sink_pad.is_linked() {
                return;
            }

            if let Err(error) = pad.link(&sink_pad) {
                eprintln!("Could not link decoded audio: {error}");
            }
        });

        Ok(Self {
            pipeline,
            video_sink,
        })
    }

    fn play(&self) {
        if let Err(error) = self.pipeline.set_state(gst::State::Playing) {
            eprintln!("Could not start playback: {error}");
        }
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

    fn install_bus_watch(&self, app: &gtk::Application) {
        let Some(bus) = self.pipeline.bus() else {
            eprintln!("Pipeline has no bus");
            return;
        };

        let app = app.clone();

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
                }

                MessageView::Warning(warning) => {
                    let source = warning
                        .src()
                        .map(|src| src.path_string().to_string())
                        .unwrap_or_else(|| "<unknown>".to_string());

                    eprintln!(
                        "GStreamer warning from {source}: {}",
                        warning.error()
                    );

                    if let Some(debug) = warning.debug() {
                        eprintln!("Debug: {debug}");
                    }
                }

                MessageView::Eos(..) => println!("End of stream"),
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

fn build_ui(
    app: &gtk::Application,
    initial_player: Rc<Player>,
) {
    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .title("MoQ GStreamer Player")
        .default_width(1280)
        .default_height(720)
        .build();

    let root = gtk::Box::new(gtk::Orientation::Vertical, 8);

    let picture = gtk::Picture::builder()
        .hexpand(true)
        .vexpand(true)
        .can_shrink(true)
        .keep_aspect_ratio(true)
        .build();

    let url_entry = gtk::Entry::builder()
        .placeholder_text("MoQ URL")
        .text(MOQ_URL)
        .build();

    let broadcast_entry = gtk::Entry::builder()
        .placeholder_text("Broadcast")
        .text(MOQ_BROADCAST)
        .build();

    let video_check = gtk::CheckButton::with_label("Enable video");
    video_check.set_active(true);

    let audio_check = gtk::CheckButton::with_label("Enable audio");
    audio_check.set_active(true);

    let track_list = gtk::Box::new(gtk::Orientation::Vertical, 4);
    track_list.append(&gtk::Label::new(Some("Discovered pads appear here.")));

    let apply_button = gtk::Button::with_label("Apply & Play");

    let panel_contents = gtk::Box::new(gtk::Orientation::Vertical, 8);
    panel_contents.set_margin_start(12);
    panel_contents.set_margin_end(12);
    panel_contents.set_margin_top(12);
    panel_contents.set_margin_bottom(12);
    panel_contents.set_width_request(280);
    panel_contents.append(&gtk::Label::new(Some("Stream settings")));
    panel_contents.append(&url_entry);
    panel_contents.append(&broadcast_entry);
    panel_contents.append(&video_check);
    panel_contents.append(&audio_check);
    panel_contents.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    panel_contents.append(&gtk::Label::new(Some("Detected pads")));
    panel_contents.append(&track_list);
    panel_contents.append(&apply_button);

    let revealer = gtk::Revealer::builder()
        .transition_type(gtk::RevealerTransitionType::SlideRight)
        .transition_duration(200)
        .reveal_child(false)
        .child(&panel_contents)
        .build();

    let picture_area = gtk::Overlay::new();
    picture_area.set_hexpand(true);
    picture_area.set_vexpand(true);
    picture_area.set_child(Some(&picture));

    let content = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    content.append(&revealer);
    content.append(&picture_area);
    content.set_vexpand(true);

    let controls = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    controls.set_margin_start(8);
    controls.set_margin_end(8);
    controls.set_margin_bottom(8);

    let settings_toggle = gtk::ToggleButton::with_label("Settings");
    let play_button = gtk::Button::with_label("Play");
    let stop_button = gtk::Button::with_label("Stop");
    let quit_button = gtk::Button::with_label("Quit");

    {
        let revealer = revealer.clone();
        settings_toggle.connect_toggled(move |button| {
            revealer.set_reveal_child(button.is_active());
        });
    }

    let current_player = Rc::new(std::cell::RefCell::new(Some(initial_player)));

    {
        let player = Rc::clone(&current_player);
        play_button.connect_clicked(move |_| {
            if let Some(player) = player.borrow().as_ref() {
                player.play();
            }
        });
    }

    {
        let player = Rc::clone(&current_player);
        stop_button.connect_clicked(move |_| {
            if let Some(player) = player.borrow().as_ref() {
                player.stop();
            }
        });
    }

    {
        let app = app.clone();
        quit_button.connect_clicked(move |_| app.quit());
    }

    {
        let player_slot = Rc::clone(&current_player);
        let app = app.clone();
        let url_entry = url_entry.clone();
        let broadcast_entry = broadcast_entry.clone();
        let video_check = video_check.clone();
        let audio_check = audio_check.clone();
        let picture = picture.clone();

        apply_button.connect_clicked(move |_| {
            let url = url_entry.text().to_string();
            let broadcast = broadcast_entry.text().to_string();

            let new_player = match Player::new(
                &url,
                &broadcast,
                video_check.is_active(),
                audio_check.is_active(),
            ) {
                Ok(player) => Rc::new(player),
                Err(error) => {
                    eprintln!("Could not create player: {error}");
                    return;
                }
            };

            new_player.install_bus_watch(&app);

            if let Some(old_player) = player_slot.borrow_mut().replace(Rc::clone(&new_player)) {
                old_player.stop();
            }

            new_player.play();

            // Poll the newly created sink until it publishes its paintable.
            let picture = picture.clone();
            let player_for_timer = Rc::clone(&new_player);

            glib::timeout_add_local(Duration::from_millis(250), move || {
                let Some(paintable) = player_for_timer.paintable() else {
                    return glib::ControlFlow::Continue;
                };

                if paintable.intrinsic_width() > 0
                    && paintable.intrinsic_height() > 0
                {
                    picture.set_paintable(Some(&paintable));
                    glib::ControlFlow::Break
                } else {
                    glib::ControlFlow::Continue
                }
            });
        });
    }

    controls.append(&settings_toggle);
    controls.append(&play_button);
    controls.append(&stop_button);
    controls.append(&quit_button);

    root.append(&content);
    root.append(&controls);
    window.set_child(Some(&root));

    {
        let app = app.clone();
        window.connect_close_request(move |_| {
            app.quit();
            glib::Propagation::Proceed
        });
    }

    window.present();

    // Attach the initial player’s paintable when it becomes available.
    let player_for_timer = Rc::clone(
        current_player.borrow().as_ref().expect("initial player exists"),
    );
    let picture = picture.clone();

    glib::timeout_add_local(Duration::from_millis(250), move || {
        let Some(paintable) = player_for_timer.paintable() else {
            return glib::ControlFlow::Continue;
        };

        if paintable.intrinsic_width() > 0
            && paintable.intrinsic_height() > 0
        {
            picture.set_paintable(Some(&paintable));
            glib::ControlFlow::Break
        } else {
            glib::ControlFlow::Continue
        }
    });
}

fn main() {
    if let Err(error) = gst::init() {
        eprintln!("Could not initialize GStreamer: {error}");
        process::exit(1);
    }

    let app = gtk::Application::builder()
        .application_id("com.cathode-ray-tube.moq-player")
        .build();

    app.connect_activate(|app| {
        let player = match Player::new(MOQ_URL, MOQ_BROADCAST, true, true) {
            Ok(player) => Rc::new(player),
            Err(error) => {
                eprintln!("Could not create player: {error}");
                app.quit();
                return;
            }
        };

        player.install_bus_watch(app);
        player.play();
        build_ui(app, player);
    });

    app.run();
}
