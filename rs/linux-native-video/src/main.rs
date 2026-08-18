use gtk::prelude::*;
use gtk::{Application, ApplicationWindow, Box as GtkBox, DrawingArea, Orientation};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

use ffmpeg_next as ffmpeg;
use ffmpeg::codec;
use ffmpeg::format::input;
use ffmpeg::frame::Video;
use ffmpeg::media::Type;
use ffmpeg::software::scaling::{context::Context as ScalingContext, flag::Flags};

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
        drawing.set_draw_func(|area, cr, width, height| {
            // Filled in by our paint below; left blank here and we repaint via closure state.
            let _ = (area, cr, width, height);
        });

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

        // Shared latest frame
        let latest_rgba: Arc<Mutex<Option<(Vec<u8>, i32, i32)>>> = Arc::new(Mutex::new(None));
        let latest_rgba_dec = latest_rgba.clone();

        let running = Arc::new(Mutex::new(true));
        let running_dec = running.clone();

        // Decode thread -> send frames occasionally; we keep only latest.
        let (tx, rx) = std::sync::mpsc::sync_channel::<(Vec<u8>, i32, i32)>(2);

        // Decode
        let status_dec = status.clone();
        thread::spawn(move || {
            if let Err(e) = decode_loop("bbb.mp4", tx, running_dec, status_dec, latest_rgba_dec) {
                eprintln!("Decoder error: {e}");
            }
        });

        // UI timer: fetch latest decoded frame and redraw
        let drawing_clone = drawing.clone();
        let latest_rgba_ui = latest_rgba.clone();

        glib::timeout_add_local(std::time::Duration::from_millis(16), move || {
            // Drain channel; keep last
            let mut got_any = false;
            while let Ok((rgba, w, h)) = rx.try_recv() {
                got_any = true;
                let mut g = latest_rgba_ui.lock().unwrap();
                *g = Some((rgba, w, h));
            }

            if got_any {
                drawing_clone.queue_draw();
            }

            glib::ControlFlow::Continue
        });

        // Stop button
        btn.connect_clicked(move |_| {
            if let Ok(mut v) = running.lock() {
                *v = false;
            }
        });

        // Actual drawing function
        drawing.connect_draw(move |_area, cr| {
            use cairo::prelude::*;

            let guard = latest_rgba.lock().unwrap();
            if let Some((rgba, w, h)) = guard.as_ref() {
                let w = *w;
                let h = *h;

                // Cairo expects ARGB32 in native endianness. We'll do a conversion RGBA->ARGB.
                // Also note: cairo::Format::ARgb32 is pre-multiplied alpha; for opaque frames, it’s fine.
                let mut argb = vec![0u8; (w as usize) * (h as usize) * 4];
                for y in 0..h as usize {
                    for x in 0..w as usize {
                        let i_rgba = (y * w as usize + x) * 4;
                        let r = rgba[i_rgba + 0];
                        let g = rgba[i_rgba + 1];
                        let b = rgba[i_rgba + 2];
                        let a = rgba[i_rgba + 3];

                        // ARGB32 = [A, R, G, B]
                        let i_argb = i_rgba;
                        argb[i_argb + 0] = a;
                        argb[i_argb + 1] = r;
                        argb[i_argb + 2] = g;
                        argb[i_argb + 3] = b;
                    }
                }

                // stride = width * 4 bytes
                let stride = w * 4;
                let surface = cairo::ImageSurface::create_for_data(
                    argb.as_mut_slice(),
                    cairo::Format::ARgb32,
                    w,
                    h,
                    stride,
                )
                .expect("create_for_data");

                cr.set_source_surface(&surface, 0.0, 0.0);
                cr.paint().expect("paint");
            }

            Inhibit(false);
        });
    });

    app.run();
}

fn decode_loop(
    path: &str,
    tx: std::sync::mpsc::SyncSender<(Vec<u8>, i32, i32)>,
    running: Arc<Mutex<bool>>,
    status: gtk::Label,
    _latest_rgba: Arc<Mutex<Option<(Vec<u8>, i32, i32)>>>,
) -> Result<(), ffmpeg::Error> {
    let mut ictx = input(&path)?;

    let input_stream = ictx
        .streams()
        .best(Type::Video)
        .ok_or(ffmpeg::Error::StreamNotFound)?;

    let stream_index = input_stream.index();

    let codec_params = input_stream.parameters();
    let codec_id = codec_params.codec_id().ok_or(ffmpeg::Error::DecoderNotFound)?;

    let codec = codec_id.and_then(codec::decoder::find).ok_or(ffmpeg::Error::DecoderNotFound)?;
    let mut decoder = codec::decoder::Decoder::open(codec)?;

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

            // Pull plane 0 bytes.
            // This may include padding; we will copy row-by-row using stride.
            let stride = out_rgba.stride(0) as usize;
            let row_bytes_src = stride;
            let row_bytes_dst = (width as usize) * 4;

            let plane0 = out_rgba.data(0);
            let src_ptr = plane0.as_ptr();
            let src_slice = unsafe {
                std::slice::from_raw_parts(src_ptr, row_bytes_src * (height as usize))
            };

            let mut rgba_bytes = vec![0u8; row_bytes_dst * (height as usize)];

            for y in 0..(height as usize) {
                let src_off = y * row_bytes_src;
                let dst_off = y * row_bytes_dst;
                rgba_bytes[dst_off..dst_off + row_bytes_dst]
                    .copy_from_slice(&src_slice[src_off..src_off + row_bytes_dst]);
            }

            let _ = tx.send((rgba_bytes, width, height));
        }
    }

    Ok(())
}
