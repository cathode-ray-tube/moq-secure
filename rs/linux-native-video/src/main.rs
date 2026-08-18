use gtk::prelude::*;
use gtk::{Application, ApplicationWindow, Box as GtkBox, DrawingArea, Orientation};
use std::sync::{Arc, Mutex};
use std::thread;

use ffmpeg_next as ffmpeg;
use ffmpeg::format::input;
use ffmpeg::frame::Video;
use ffmpeg::media::Type;
use ffmpeg::software::scaling::{context::Context as ScalingContext, flag::Flags};

use gtk::cairo;

use async_channel::{bounded, Receiver, Sender};

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
        drawing.set_hexpand(true);
        drawing.set_vexpand(true);
        drawing.set_content_width(video_w);
        drawing.set_content_height(video_h);

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

        let running: Arc<Mutex<bool>> = Arc::new(Mutex::new(true));
        let running_dec = running.clone();

        // Status updates (async)
        let (status_tx, status_rx): (Sender<String>, Receiver<String>) = bounded(50);
        let status_label = status.clone();
        gtk::glib::MainContext::default().spawn_local(async move {
            while let Ok(msg) = status_rx.recv().await {
                status_label.set_text(&msg);
            }
        });

        // Redraw requests (async)
        let (redraw_tx, redraw_rx): (Sender<()>, Receiver<()>) = bounded(100);
        let drawing_for_redraw = drawing.clone();
        gtk::glib::MainContext::default().spawn_local(async move {
            while let Ok(_) = redraw_rx.recv().await {
                drawing_for_redraw.queue_draw();
            }
        });

        // Draw func
        let latest_ui = latest.clone();
        drawing.set_draw_func(move |_area, cr, width, height| {
            if let Some((rgba, w, h)) = latest_ui.lock().unwrap().as_ref() {
                draw_rgba_as_argb32_scaled(cr, rgba, *w, *h, width as i32, height as i32);
            }
        });

        // Decoder thread
        thread::spawn(move || {
            if let Err(e) = decode_loop(
                "bbb.mp4",
                running_dec,
                latest_dec,
                status_tx,
                redraw_tx,
            ) {
                eprintln!("Decoder error: {e}");
            }
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

fn draw_rgba_as_argb32_scaled(
    cr: &cairo::Context,
    rgba: &[u8], // [R,G,B,A]
    src_w: i32,
    src_h: i32,
    dst_w: i32,
    dst_h: i32,
) {
    if src_w <= 0 || src_h <= 0 || dst_w <= 0 || dst_h <= 0 {
        return;
    }

    let src_w_usize = src_w as usize;
    let src_h_usize = src_h as usize;

    // Convert RGBA -> BGRA bytes for cairo ARGB32 on little-endian systems.
    let mut bgra = vec![0u8; src_w_usize * src_h_usize * 4];
    for y in 0..src_h_usize {
        for x in 0..src_w_usize {
            let i = (y * src_w_usize + x) * 4;
            let r = rgba[i + 0];
            let g = rgba[i + 1];
            let b = rgba[i + 2];
            let a = rgba[i + 3];

            bgra[i + 0] = b; // B
            bgra[i + 1] = g; // G
            bgra[i + 2] = r; // R
            bgra[i + 3] = a; // A
        }
    }

    let mut surface =
        cairo::ImageSurface::create(cairo::Format::ARgb32, src_w, src_h)
            .expect("create surface");
    let stride = surface.stride() as usize;
    let row_bytes = src_w_usize * 4;

    {
        let mut data = surface.data().expect("surface.data() failed");
        for y in 0..src_h_usize {
            let dst_off = y * stride;
            let src_off = y * row_bytes;
            data[dst_off..dst_off + row_bytes]
                .copy_from_slice(&bgra[src_off..src_off + row_bytes]);
        }
    }

    let sx = dst_w as f64 / src_w as f64;
    let sy = dst_h as f64 / src_h as f64;

    cr.save();
    cr.scale(sx, sy);
    cr.set_source_surface(&surface, 0.0, 0.0);
    cr.paint().ok();
    cr.restore();
}

fn decode_loop(
    path: &str,
    running: Arc<Mutex<bool>>,
    latest: Arc<Mutex<Option<(Vec<u8>, i32, i32)>>>,
    status_tx: Sender<String>,
    redraw_tx: Sender<()>,
) -> Result<(), ffmpeg::Error> {
    let mut ictx = input(&path)?;

    let input_stream = ictx
        .streams()
        .best(Type::Video)
        .ok_or(ffmpeg::Error::StreamNotFound)?;

    let stream_index = input_stream.index();
    let time_base = input_stream.time_base(); // seconds = pts * num / den

    // FPS fallback when timestamps are missing
    let mut fps_fallback = 0.0;
    let avg_frame_rate = input_stream.avg_frame_rate();
    if avg_frame_rate.1 != 0 {
        fps_fallback = avg_frame_rate.0 as f64 / avg_frame_rate.1 as f64;
    }
    if fps_fallback <= 0.0 {
        fps_fallback = 30.0;
    }
    let frame_duration = 1.0 / fps_fallback;

    let codec_params = input_stream.parameters();
    let codec_id = codec_params.id();
    if codec_id == ffmpeg::codec::Id::None {
        return Err(ffmpeg::Error::DecoderNotFound);
    }

    let decoder_codec = ffmpeg::codec::decoder::find(codec_id)
        .ok_or(ffmpeg::Error::DecoderNotFound)?;

    let mut context = ffmpeg::codec::Context::new();
    context.set_parameters(codec_params)?;
    let mut decoder = context.decoder().open_as(decoder_codec)?;

    let mut scaler: Option<ScalingContext> = None;
    let mut out_rgba: Video = Video::empty();

    // Pacing state
    let mut started = false;
    let mut start_instant = std::time::Instant::now();
    let mut start_pts: i64 = 0;

    // If timestamps are absent, pace by frame index
    let mut frame_index: i64 = 0;

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

            if !started {
                started = true;
                start_instant = std::time::Instant::now();
                start_pts = decoded.timestamp().unwrap_or(0);
                let _ = status_tx.try_send("Status: playing (timed) ...".to_string());
            }

            // --- Real-time pacing ---
            if let Some(pts) = decoded.timestamp() {
                let elapsed_pts = pts - start_pts;
                let elapsed_secs =
                    (elapsed_pts as f64) * (time_base.0 as f64) / (time_base.1 as f64);

                let target = start_instant + std::time::Duration::from_secs_f64(elapsed_secs.max(0.0));
                let now = std::time::Instant::now();
                if target > now {
                    std::thread::sleep(target - now);
                }
            } else {
                // No timestamp: fallback pacing
                let target_elapsed = (frame_index as f64) * frame_duration;
                let target = start_instant + std::time::Duration::from_secs_f64(target_elapsed.max(0.0));
                let now = std::time::Instant::now();
                if target > now {
                    std::thread::sleep(target - now);
                }
                frame_index += 1;
            }

            // --- Decode -> scale -> copy RGBA ---
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

                let _ = status_tx.try_send(format!(
                    "Status: playing ({}x{}) ...",
                    src_w, src_h
                ));
            }

            let ctx = scaler.as_mut().unwrap();
            ctx.run(&decoded, &mut out_rgba)?;

            let width_u32 = out_rgba.width();
            let height_u32 = out_rgba.height();

            let width: i32 = width_u32.try_into().unwrap_or(0);
            let height: i32 = height_u32.try_into().unwrap_or(0);

            let plane0 = out_rgba.data(0);
            let stride_src = out_rgba.stride(0) as usize;
            let row_bytes_dst = (width as usize) * 4;
            let height_usize = height as usize;

            let src_ptr = plane0.as_ptr();
            let src_len = stride_src * height_usize;
            let src_slice = unsafe { std::slice::from_raw_parts(src_ptr, src_len) };

            // Copy into tightly packed RGBA (width*4 bytes per row)
            let mut rgba_bytes = vec![0u8; row_bytes_dst * height_usize];
            for y in 0..height_usize {
                let src_off = y * stride_src;
                let dst_off = y * row_bytes_dst;
                rgba_bytes[dst_off..dst_off + row_bytes_dst]
                    .copy_from_slice(&src_slice[src_off..src_off + row_bytes_dst]);
            }

            if let Ok(mut g) = latest.lock() {
                *g = Some((rgba_bytes, width, height));
            }

            let _ = redraw_tx.try_send(());
        }
    }

    Ok(())
}
