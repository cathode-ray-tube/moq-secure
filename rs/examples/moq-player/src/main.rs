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

fn layout_tile_count(layout: usize) -> usize {
    match layout {
        0 => 3, // Three portrait tiles
        1 => 4, // 2x2 square tiles
        2 => 3, // Main plus two stacked
        _ => 1, // Single full-size tile
    }
}

fn tile_position(layout: usize, tile: usize) -> (i32, i32, i32, i32) {
    match layout {
        // Three portrait tiles, side by side.
        0 => (tile as i32, 0, 1, 1),

        // Four-tile 2x2 grid.
        1 => ((tile % 2) as i32, (tile / 2) as i32, 1, 1),

        // Large main tile on the left; two stacked tiles on the right.
        2 => match tile {
            0 => (0, 0, 2, 2),
            1 => (2, 0, 1, 1),
            _ => (2, 1, 1, 1),
        },

        // One full-size tile.
        _ => (0, 0, 1, 1),
    }
}

fn layout_diagram(layout: usize) -> gtk::Widget {
    let diagram = gtk::Grid::builder()
        .row_spacing(2)
        .column_spacing(2)
        .width_request(68)
        .height_request(44)
        .build();

    let tile = |label: &str| {
        let frame = gtk::Frame::new(None);
        frame.set_child(Some(&gtk::Label::new(Some(label))));
        frame
    };

    match layout {
        0 => {
            for col in 0..3 {
                diagram.attach(&tile("▯"), col, 0, 1, 1);
            }
        }
        1 => {
            for row in 0..2 {
                for col in 0..2 {
                    diagram.attach(&tile("□"), col, row, 1, 1);
                }
            }
        }
        2 => {
            diagram.attach(&tile("MAIN"), 0, 0, 2, 2);
            diagram.attach(&tile("1"), 2, 0, 1, 1);
            diagram.attach(&tile("2"), 2, 1, 1, 1);
        }
        _ => {
            diagram.attach(&tile("FULL"), 0, 0, 1, 1);
        }
    }

    diagram.upcast()
}

fn build_ui(app: &gtk::Application, initial_player: Rc<Player>) {
    use std::cell::RefCell;

    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .title("MoQ GStreamer Player")
        .default_width(1280)
        .default_height(720)
        .build();

    let root = gtk::Box::new(gtk::Orientation::Vertical, 8);

    let content = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    content.set_hexpand(true);
    content.set_vexpand(true);

    let tile_grid = gtk::Grid::builder()
        .hexpand(true)
        .vexpand(true)
        .row_spacing(4)
        .column_spacing(4)
        .build();

    let settings_panel = gtk::Box::new(gtk::Orientation::Vertical, 8);
    settings_panel.set_width_request(300);
    settings_panel.set_margin_start(12);
    settings_panel.set_margin_end(12);
    settings_panel.set_margin_top(12);
    settings_panel.set_margin_bottom(12);

    let settings_revealer = gtk::Revealer::builder()
        .transition_type(gtk::RevealerTransitionType::SlideRight)
        .transition_duration(200)
        .reveal_child(false)
        .child(&settings_panel)
        .build();

    content.append(&settings_revealer);
    content.append(&tile_grid);

    let controls = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    controls.set_margin_start(8);
    controls.set_margin_end(8);
    controls.set_margin_bottom(8);

    let settings_button = gtk::ToggleButton::with_label("Settings");
    let play_button = gtk::Button::with_label("Play");
    let stop_button = gtk::Button::with_label("Stop");
    let quit_button = gtk::Button::with_label("Quit");

    controls.append(&settings_button);
    controls.append(&play_button);
    controls.append(&stop_button);
    controls.append(&quit_button);

    root.append(&content);
    root.append(&controls);
    window.set_child(Some(&root));

    let current_players: Rc<RefCell<Vec<Rc<Player>>>> =
        Rc::new(RefCell::new(vec![initial_player]));

    let selected_layout = Rc::new(Cell::new(3usize));

    // Inputs are rebuilt when the user selects a different layout.
    let tile_inputs: Rc<RefCell<Vec<(gtk::Entry, gtk::Entry)>>> =
        Rc::new(RefCell::new(Vec::new()));

    let rebuild_settings: Rc<RefCell<Option<Rc<dyn Fn()>>>> =
        Rc::new(RefCell::new(None));

    {
        let settings_revealer = settings_revealer.clone();
        settings_button.connect_toggled(move |button| {
            settings_revealer.set_reveal_child(button.is_active());
        });
    }

    {
        let players = Rc::clone(&current_players);
        play_button.connect_clicked(move |_| {
            for player in players.borrow().iter() {
                player.play();
            }
        });
    }

    {
        let players = Rc::clone(&current_players);
        stop_button.connect_clicked(move |_| {
            for player in players.borrow().iter() {
                player.stop();
            }
        });
    }

    {
        let app = app.clone();
        quit_button.connect_clicked(move |_| app.quit());
    }

    // Rebuild the side panel when a layout is selected.
    {
        let panel = settings_panel.clone();
        let grid = tile_grid.clone();
        let selected_layout_for_apply = Rc::clone(&selected_layout);
        let tile_inputs = Rc::clone(&tile_inputs);
        let rebuild_settings = Rc::clone(&rebuild_settings);
        let current_players = Rc::clone(&current_players);
        let app = app.clone();

        let rebuild: Rc<dyn Fn()> = Rc::new(move || {
            while let Some(child) = panel.first_child() {
                panel.remove(&child);
            }

            let layout = selected_layout_for_apply.get();
            let count = layout_tile_count(layout);

            panel.append(&gtk::Label::new(Some("Tile layout")));

            let layout_buttons = gtk::Box::new(gtk::Orientation::Horizontal, 4);
            let mut buttons = Vec::new();

            for index in 0..4 {
                let button = gtk::ToggleButton::new();
                button.set_child(Some(&layout_diagram(index)));
                button.set_tooltip_text(Some(match index {
                    0 => "Three portrait tiles",
                    1 => "Four square tiles",
                    2 => "One large tile and two stacked tiles",
                    _ => "One full-screen tile",
                }));
                buttons.push(button);
            }

            for (index, button) in buttons.iter().enumerate() {
                layout_buttons.append(button);

                let selected_layout = Rc::clone(&selected_layout);
                let rebuild_settings = Rc::clone(&rebuild_settings);

                button.connect_clicked(move |_| {
                    selected_layout.set(index);

                    if let Some(rebuild) = rebuild_settings.borrow().as_ref() {
                        rebuild();
                    }
                });
            }

            buttons[layout].set_active(true);
            panel.append(&layout_buttons);

            panel.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
            panel.append(&gtk::Label::new(Some("Stream inputs")));

            let mut entries = Vec::new();

            for tile in 0..count {
                let heading = if layout == 2 && tile == 0 {
                    "Main tile"
                } else {
                    match tile {
                        0 => "Tile 1",
                        1 => "Tile 2",
                        2 => "Tile 3",
                        _ => "Tile 4",
                    }
                };

                panel.append(&gtk::Label::new(Some(heading)));

                let url = gtk::Entry::builder()
                    .placeholder_text("MoQ URL")
                    .text(MOQ_URL)
                    .build();

                let broadcast = gtk::Entry::builder()
                    .placeholder_text("Broadcast")
                    .text(MOQ_BROADCAST)
                    .build();

                panel.append(&url);
                panel.append(&broadcast);
                entries.push((url, broadcast));
            }

            *tile_inputs.borrow_mut() = entries;

            let apply = gtk::Button::with_label("Apply & Play");
            panel.append(&apply);

            {
                let entries = Rc::clone(&tile_inputs);
                let players = Rc::clone(&current_players);
                let app = app.clone();
                let grid = grid.clone();

                apply.connect_clicked(move |_| {
                    let mut new_players = Vec::new();

                    for (index, (url, broadcast)) in
                        entries.borrow().iter().enumerate()
                    {
                        match Player::new(&url.text(), &broadcast.text(), true, true) {
                            Ok(player) => {
                                let player = Rc::new(player);
                                player.install_bus_watch(&app);
                                new_players.push(player);
                            }
                            Err(error) => {
                                eprintln!(
                                    "Could not create player for tile {}: {error}",
                                    index + 1
                                );
                                return;
                            }
                        }
                    }

                    for old_player in players.borrow().iter() {
                        old_player.stop();
                    }

                    *players.borrow_mut() = new_players;

                    while let Some(child) = grid.first_child() {
                        grid.remove(&child);
                    }

                    let layout = selected_layout.get();

                    for (index, player) in players.borrow().iter().enumerate() {
                        let picture = gtk::Picture::builder()
                            .hexpand(true)
                            .vexpand(true)
                            .can_shrink(true)
                            .keep_aspect_ratio(true)
                            .build();

                        let frame = gtk::Frame::new(None);
                        frame.set_child(Some(&picture));
                        frame.set_hexpand(true);
                        frame.set_vexpand(true);

                        let (col, row, width, height) =
                            tile_position(layout, index);

                        grid.attach(&frame, col, row, width, height);

                       let picture = picture.clone();
                        let player_for_timeout = Rc::clone(player);
                        
                        glib::timeout_add_local(Duration::from_millis(250), move || {
                            let Some(paintable) = player_for_timeout.paintable() else {
                                return glib::ControlFlow::Continue;
                            };
                        
                            if paintable.intrinsic_width() > 0 && paintable.intrinsic_height() > 0 {
                                picture.set_paintable(Some(&paintable));
                                glib::ControlFlow::Break
                            } else {
                                glib::ControlFlow::Continue
                            }
                        });


                        player.play();
                    }
                });
            }
        });

        *rebuild_settings.borrow_mut() = Some(Rc::clone(&rebuild));
        rebuild();
    }

    // Show the initial player in the initial one-tile layout.
    {
        let picture = gtk::Picture::builder()
            .hexpand(true)
            .vexpand(true)
            .can_shrink(true)
            .keep_aspect_ratio(true)
            .build();

        let frame = gtk::Frame::new(None);
        frame.set_child(Some(&picture));
        frame.set_hexpand(true);
        frame.set_vexpand(true);
        tile_grid.attach(&frame, 0, 0, 1, 1);

        let player = Rc::clone(&current_players.borrow()[0]);
        glib::timeout_add_local(Duration::from_millis(250), move || {
            let Some(paintable) = player.paintable() else {
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