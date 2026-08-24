use std::{process, rc::Rc, time::Duration};

use gtk4 as gtk;
use gtk::{glib, prelude::*};

use gstreamer as gst;
use gst::prelude::*;

const MOQ_URL: &str = "https://cdn.moq.dev/demo";
const MOQ_BROADCAST: &str = "bbb.hang";

// Positive value delays audio.
// Negative value advances audio.
const AUDIO_TS_OFFSET_NS: i64 = 0;

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

        // -------------------------------------------------------------
        // Video elements
        // -------------------------------------------------------------

        let video_parser = gst::ElementFactory::make("h264parse")
            .property("config-interval", 1i32)
            .build()?;

        // Decouples the MoQ source/parser from the video decoder.
        let video_queue = gst::ElementFactory::make("queue")
            .property("max-size-time", 500_000_000u64)
            .property("max-size-buffers", 0u32)
            .property("max-size-bytes", 0u32)
            .build()?;

        video_queue.set_property_from_str("leaky", "no");

        let video_decoder = gst::ElementFactory::make("decodebin3").build()?;

        // Decouples decoding from conversion and GTK rendering.
        let decoded_video_queue = gst::ElementFactory::make("queue")
            .property("max-size-time", 500_000_000u64)
            .property("max-size-buffers", 0u32)
            .property("max-size-bytes", 0u32)
            .build()?;

        decoded_video_queue.set_property_from_str("leaky", "no");

        let video_convert = gst::ElementFactory::make("videoconvert").build()?;

        // Set sync=false while diagnosing live-stream timestamp problems.
        // Once playback is stable, try changing both sync properties to true.
        let video_sink = gst::ElementFactory::make("gtk4paintablesink")
            .property("sync", false)
            .property("async", false)
            .build()
            .map_err(|e| format!("Could not create gtk4paintablesink: {e}"))?;

        // -------------------------------------------------------------
        // Audio elements
        // -------------------------------------------------------------

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

        // -------------------------------------------------------------
        // Add elements
        // -------------------------------------------------------------

        pipeline.add_many([
            &source,

            // Video
            &video_parser,
            &video_queue,
            &video_decoder,
            &decoded_video_queue,
            &video_convert,
            &video_sink,

            // Audio
            &audio_parser,
            &audio_decoder,
            &audio_queue,
            &audio_convert,
            &audio_resample,
            &audio_caps,
            &audio_volume,
            &audio_sink,
        ])?;

        // -------------------------------------------------------------
        // Static links
        // -------------------------------------------------------------

        // Encoded video:
        // moqsrc → h264parse → queue → decodebin3
        gst::Element::link_many([
            &video_parser,
            &video_queue,
            &video_decoder,
        ])?;

        // Decoded video:
        // decoded queue → videoconvert → gtk4paintablesink
        gst::Element::link_many([
            &decoded_video_queue,
            &video_convert,
            &video_sink,
        ])?;

        // Encoded audio:
        // moqsrc → aacparse → decodebin3
        gst::Element::link_many([
            &audio_parser,
            &audio_decoder,
        ])?;

        // Decoded audio:
        // queue → audioconvert → audioresample → capsfilter
        //       → volume → pipewiresink
        gst::Element::link_many([
            &audio_queue,
            &audio_convert,
            &audio_resample,
            &audio_caps,
            &audio_volume,
            &audio_sink,
        ])?;

        // -------------------------------------------------------------
        // Route dynamic pads from moqsrc
        // -------------------------------------------------------------

        let video_parser_weak = video_parser.downgrade();
        let audio_parser_weak = audio_parser.downgrade();

        source.connect_pad_added(move |_source, pad| {
            if pad.is_linked() {
                return;
            }

            let caps = pad
                .current_caps()
                .unwrap_or_else(|| pad.query_caps(None));

            let Some(structure) = caps.structure(0) else {
                eprintln!("MoQ pad has no caps: {caps}");
                return;
            };

            let media_type = structure.name();

            let target = if media_type.starts_with("video/") {
                let Some(parser) = video_parser_weak.upgrade() else {
                    return;
                };

                parser
            } else if media_type.starts_with("audio/") {
                let Some(parser) = audio_parser_weak.upgrade() else {
                    return;
                };

                parser
            } else {
                eprintln!("Ignoring unsupported MoQ caps: {caps}");
                return;
            };

            let Some(target_sink) = target.static_pad("sink") else {
                eprintln!("Target has no sink pad for {media_type}");
                return;
            };

            if target_sink.is_linked() {
                eprintln!("Target sink is already linked: {media_type}");
                return;
            }

            match pad.link(&target_sink) {
                Ok(_) => {
                    println!("Linked MoQ {media_type} pad");
                }

                Err(error) => {
                    eprintln!("Could not link MoQ {media_type} pad: {error}");
                }
            }
        });

        // -------------------------------------------------------------
        // Route decoded video pads
        // -------------------------------------------------------------

        let decoded_video_queue_weak = decoded_video_queue.downgrade();

        video_decoder.connect_pad_added(move |_decoder, pad| {
            let caps = pad
                .current_caps()
                .unwrap_or_else(|| pad.query_caps(None));

            println!("Decoded video pad {} caps: {caps}", pad.name());

            let Some(structure) = caps.structure(0) else {
                eprintln!("Decoded video pad has no caps");
                return;
            };

            if !structure.name().starts_with("video/") {
                return;
            }

            let Some(decoded_video_queue) = decoded_video_queue_weak.upgrade() else {
                return;
            };

            let Some(sink_pad) = decoded_video_queue.static_pad("sink") else {
                eprintln!("Decoded video queue has no sink pad");
                return;
            };

            if sink_pad.is_linked() {
                eprintln!("Decoded video queue sink pad is already linked");
                return;
            }

            match pad.link(&sink_pad) {
                Ok(_) => {
                    println!("Linked decoded video to video queue");
                }

                Err(error) => {
                    eprintln!("Could not link decoded video: {error}");
                }
            }
        });

        // -------------------------------------------------------------
        // Route decoded audio pads
        // -------------------------------------------------------------

        let audio_queue_weak = audio_queue.downgrade();

        audio_decoder.connect_pad_added(move |_decoder, pad| {
            let caps = pad
                .current_caps()
                .unwrap_or_else(|| pad.query_caps(None));

            println!("Decoded audio pad {} caps: {caps}", pad.name());

            let Some(structure) = caps.structure(0) else {
                eprintln!("Decoded audio pad has no caps");
                return;
            };

            if !structure.name().starts_with("audio/") {
                return;
            }

            let Some(audio_queue) = audio_queue_weak.upgrade() else {
                return;
            };

            let Some(sink_pad) = audio_queue.static_pad("sink") else {
                eprintln!("Audio queue has no sink pad");
                return;
            };

            if sink_pad.is_linked() {
                eprintln!("Audio queue sink pad is already linked");
                return;
            }

            match pad.link(&sink_pad) {
                Ok(_) => {
                    println!("Linked decoded audio to audio queue");
                }

                Err(error) => {
                    eprintln!("Could not link decoded audio: {error}");
                }
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

                    eprintln!(
                        "GStreamer error from {source}: {}",
                        error.error()
                    );

                    if let Some(debug) = error.debug() {
                        eprintln!("Debug: {debug}");
                    }

                    app.quit();
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

                MessageView::Eos(..) => {
                    println!("End of stream");
                    app.quit();
                }

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

fn build_ui(app: &gtk::Application, player: Rc<Player>) {
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

    let controls = gtk::Box::new(gtk::Orientation::Horizontal, 8);

    let play_button = gtk::Button::with_label("Play");
    let stop_button = gtk::Button::with_label("Stop");
    let quit_button = gtk::Button::with_label("Quit");

    {
        let player = Rc::clone(&player);

        play_button.connect_clicked(move |_| {
            player.play();
        });
    }

    {
        let player = Rc::clone(&player);

        stop_button.connect_clicked(move |_| {
            player.stop();
        });
    }

    {
        let app = app.clone();

        quit_button.connect_clicked(move |_| {
            app.quit();
        });
    }

    controls.append(&play_button);
    controls.append(&stop_button);
    controls.append(&quit_button);

    root.append(&picture);
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

    // gtk4paintablesink exposes its paintable after the first video frame.
    let picture = picture.clone();
    let player_for_timer = Rc::clone(&player);

    glib::timeout_add_local(Duration::from_millis(250), move || {
        let Some(paintable) = player_for_timer.paintable() else {
            return glib::ControlFlow::Continue;
        };

        if paintable.intrinsic_width() > 0
            && paintable.intrinsic_height() > 0
        {
            picture.set_paintable(Some(&paintable));

            println!(
                "Video paintable ready: {}x{}",
                paintable.intrinsic_width(),
                paintable.intrinsic_height()
            );

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
        .application_id("com.example.moq-gstreamer-player")
        .build();

    app.connect_activate(|app| {
        let player = match Player::new(MOQ_URL, MOQ_BROADCAST) {
            Ok(player) => Rc::new(player),

            Err(error) => {
                eprintln!("Could not create player: {error}");
                app.quit();
                return;
            }
        };

        player.install_bus_watch(app);
        build_ui(app, Rc::clone(&player));

        let player_for_start = Rc::clone(&player);

        glib::idle_add_local_once(move || {
            player_for_start.play();
        });
    });

    app.run();
}
