use gtk::prelude::*;
use gtk::{Application, ApplicationWindow, Box as GtkBox, DrawingArea, Orientation};
use std::sync::{Arc, Mutex};
use std::thread;

// ffmpeg
use ffmpeg_next as ffmpeg;
use ffmpeg::codec;
use ffmpeg::format::input;
use ffmpeg::frame::Video;
use ffmpeg::media::Type;
use ffmpeg::software::scaling::{context::Context as ScalingContext, flag::Flags};

// IMPORTANT: Use cairo re-export from gtk4 so the Context type matches GTK's draw callback
use gtk::cairo;

fn main() {
    ffmpeg::init().unwrap();

    let app = Application::builder()
        .application_id("com.example.ffmpeg-gtk-cairo")
        .build();

    app.connect_activate(|app| {
        let window = ApplicationWindow::builder()
            .application(app)
            .default_width(1000)
            .default_height(720)
            .title("FFmpeg + GTK4 (cairo, no OpenGL)")
            .build();

        let root = GtkBox::new(Orientation::Horizontal, 12);

        let video_w: i32 = 360;
        let video_h: i32 = 640;

        let drawing = DrawingArea::new();
        drawing.set_content_width(video_w);
        drawing.set_content_height(video_h);
        drawing.set_hexpand(false);
        drawing.set_vexpand(false);

        let status = gtk::Label::new(Some("Status: decoding bbb.mp4 ..."));
        let btn = gtk::Button::with_label("Stop");

        let side = GtkBox::new(Orientation::Vertical, 10);
        side.set_size_request(300, -1);
        side.append(&status);
        side.append(&btn);

        root.append(&drawing);
        root.append(&side);

        window.set_child(Some(&root));
        window.show();

        let latest: Arc<Mutex<Option<(Vec<u8>, i32, i32)>>> = Arc::new(Mutex::new(None));
        let latest_dec = latest.clone();

        let running = Arc::new(Mutex::new(true));
        let running_dec = running.clone();

        let (tx, rx) =
            std::sync::mpsc::sync_channel::<(Vec<u8>, i32, i32)>(2);

        // Decoder thread
        let status_dec = status.clone();
        thread::spawn(move || {
            if let Err(e) = decode_loop("bbb.mp4", tx, running_dec, status_dec, latest_dec) {
                eprintln!("Decoder error: {e}");
            }
        });

        // Drawing
        let latest_ui = latest.clone();
        drawing.set_draw_func(move |_area, cr, _width, _height| {
            if let Some((rgba, w, h)) = latest_ui.lock().unwrap().as_ref() {
                draw_rgba_as_argb32_rgba_prefilled(cr, rgba, *w, *h);
            }
        });

        // Poll frames
        let drawing_for_redraw = drawing.clone();
        glib::timeout_add_local(std::time::Duration::from_millis(16), move || {
            let mut got = false;
            while let Ok((rgba, w, h)) = rx.try_recv() {
                got = true;
                *latest.lock().unwrap() = Some((rgba, w, h));
            }
            if got {
                drawing_for_redraw.queue_draw();
            }
            glib::ControlFlow::Continue
        });

        // Stop
        btn.connect_clicked(move |_| {
            if let Ok(mut v) = running.lock() {
                *v = false;
            }
        });
    });

    app.run();
}

fn draw_rgba_as_argb32_rgba_prefilled(cr: &cairo::Context, rgba: &[u8], w: i32, h: i32) {
    let w_usize = w as usize;
    let h_usize = h as usize;

    // Input rgba: [R,G,B,A]
    // Output cairo ARGB32 (using same mapping you had):
    // argb[i+0]=A, argb[i+1]=R, argb[i+2]=G, argb[i+3]=B
    let mut argb = vec![0u8; w_usize * h_usize * 4];
    for y in 0..h_usize {
        for x in 0..w_usize {
            let i = (y * w_usize + x) * 4;
            let r = rgba[i + 0];
            let g = rgba[i + 1];
            let b = rgba[i + 2];
            let a = rgba[i + 3];

            argb[i + 0] = a;
            argb[i + 1] = r;
            argb[i + 2] = g;
            argb[i + 3] = b;
        }
    }

    // Create surface with owned buffer (fix for E0597)
    let surface =
        cairo::ImageSurface::create(cairo::Format::ARgb32, w, h).expect("create surface");

    let data = surface.data().expect("surface.data");
    // ARGB32 stride in bytes is typically w*4, but use surface's stride:
    let stride = surface.stride() as usize;

    for y in 0..h_usize {
        let dst_off = y * stride;
        let src_off = y * w_usize * 4;
        data[dst_off..dst_off + w_usize * 4]
            .copy_from_slice(&argb[src_off..src_off + w_usize * 4]);
    }

    cr.set_source_surface(&surface, 0.0, 0.0);
    cr.paint().ok();
}

fn decode_loop(
    path: &str,
    tx: std::sync::mpsc::SyncSender<(Vec<u8>, i32, i32)>,
    running: Arc<Mutex<bool>>,
    status: gtk::Label,
    latest: Arc<Mutex<Option<(Vec<u8>, i32, i32)>>>,
) -> Result<(), ffmpeg::Error> {
    let mut ictx = input(&path)?;

    let input_stream = ictx
        .streams()
        .best(Type::Video)
        .ok_or(ffmpeg::Error::StreamNotFound)?;

    let stream_index = input_stream.index();
    let codec_params = input_stream.parameters();

    // Fix for E0599: use codec_id()
    let codec_id = codec_params.codec_id();
    let decoder_codec = codec::decoder::find(codec_id)
        .ok_or(ffmpeg::Error::DecoderNotFound)?;
    let mut decoder = decoder_codec.decoder().open_as(decoder_codec)?;

    let mut scaler: Option<ScalingContext> = None;
    let mut out_rgba: Video = Video::empty();

    for (stream, packet) in ictx.packets() {
        if !*running.lock().unwrap() {
            break;
        }
        if stream.index() != stream_index {
            continue;
        }

        decoder.send_packet(&packet)?;

        let mut decoded = Video::empty();
        while decoder.receive_frame(&mut decoded).is_ok() {
            if !*running.lock().unwrap() {
                break;
            }

            let src_w = decoded.width();
            let src_h = decoded.height();

            if scaler.is_none() {
                let src_format = decoded.format();
                let dst_format = ffmpeg::format::Pixel::RGBA;

                let ctx = ScalingContext::get(
                    src_format,
                    src_w,
                    src_h,
                    dst_format,
                    src_w,
                    src_h,
                    Flags::BILINEAR,
                )?;

                scaler = Some(ctx);
                out_rgba = Video::empty();

                status.set_text(&format!("Status: decoding ({}x{}) ...", src_w, src_h));
            }

            let ctx = scaler.as_mut().unwrap();
            ctx.run(&decoded, &mut out_rgba)?;

            let width = out_rgba.width();
            let height = out_rgba.height();

            let plane0 = out_rgba.data(0);
            let stride_src = out_rgba.stride(0) as usize;
            let row_bytes_dst = (width as usize) * 4;
            let height_usize = height as usize;

            let src_ptr = plane0.as_ptr();
            let src_len = stride_src * height_usize;
            let src_slice = unsafe { std::slice::from_raw_parts(src_ptr, src_len) };

            let mut rgba_bytes = vec![0u8; row_bytes_dst * height_usize];
            for y in 0..height_usize {
                let src_off = y * stride_src;
                let dst_off = y * row_bytes_dst;
                rgba_bytes[dst_off..dst_off + row_bytes_dst]
                    .copy_from_slice(&src_slice[src_off..src_off + row_bytes_dst]);
            }

            if let Ok(mut g) = latest.lock() {
                *g = Some((rgba_bytes.clone(), width as i32, height as i32));
            }

            let _ = tx.send((rgba_bytes, width as i32, height as i32));
        }
    }

    Ok(())
}
