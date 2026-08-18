use gtk::prelude::*;
use gtk::{
    Application, ApplicationWindow, Box as GtkBox, DrawingArea, Orientation,
};
use std::sync::{Arc, Mutex};
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

        // Latest frame (RGBA bytes + w/h)
        let latest: Arc<Mutex<Option<(Vec<u8>, i32, i32)>>> = Arc::new(Mutex::new(None));
        let latest_dec = latest.clone();

        let running = Arc::new(Mutex::new(true));
        let running_dec = running.clone();

        // Latest-only sync channel
        let (tx, rx) = std::sync::mpsc::sync_channel::<(Vec<u8>, i32, i32)>(2);

        // Decoder thread
        let status_dec = status.clone();
        thread::spawn(move || {
            if let Err(e) = decode_loop(
                "bbb.mp4",
                tx,
                running_dec,
                status_dec,
                latest_dec,
            ) {
                eprintln!("Decoder error: {e}");
            }
        });

        // UI timer: move latest from rx into shared "latest" and redraw
        let latest_ui = latest.clone();
        drawing.set_draw_func(move |area, cr, width, height| {
            // draw func uses latest; actual redraw triggered by timeout/queue_draw
            let _ = (area, width, height);

            let guard = latest_ui.lock().unwrap();
            if let Some((rgba, w, h)) = guard.as_ref() {
                draw_rgba_as_argb32_rgba_prefilled(cr, rgba, *w, *h);
            }
        });

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

fn draw_rgba_as_argb32_rgba_prefilled(
    cr: &cairo::Context,
    rgba: &[u8],
    w: i32,
    h: i32,
) {
    // Cairo ARGB32 expects bytes in native endianness; simplest portable assumption:
    // Use [A, R, G, B] layout.
    let mut argb = vec![0u8; (w as usize) * (h as usize) * 4];
    for y in 0..(h as usize) {
        for x in 0..(w as usize) {
            let i = (y * (w as usize) + x) * 4;
            let r = rgba[i + 0];
            let g = rgba[i + 1];
            let b = rgba[i + 2];
            let a = rgba[i + 3];

            argb[i + 0] = a; // A
            argb[i + 1] = r; // R
            argb[i + 2] = g; // G
            argb[i + 3] = b; // B
        }
    }

    let stride = (w as i64 * 4) as i32;

    // cairo::ImageSurface requires stable memory during paint;
    // we keep argb alive by drawing immediately.
    let surface = cairo::ImageSurface::create_for_data(
        argb.as_mut_slice(),
        cairo::Format::ARgb32,
        w,
        h,
        stride as i32,
    )
    .expect("create_for_data failed");

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
    use ffmpeg::software::scaling;

    let mut ictx = input(&path)?;

    let input_stream = ictx
        .streams()
        .best(Type::Video)
        .ok_or(ffmpeg::Error::StreamNotFound)?;

    let stream_index = input_stream.index();

    let codec_params = input_stream.parameters();

    // Your bindings: Parameters has no `codec_id()`.
    // Try `codec_params.codec()` first (common in ffmpeg-next bindings).
    let codec = if let Some(c) = codec_params.codec() {
        c
    } else {
        // Fallback: if codec() is not available in your exact build, paste the error and
        // I’ll adjust this block precisely for your ffmpeg-next version.
        return Err(ffmpeg::Error::DecoderNotFound);
    };

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

                status.set_text(&format!(
                    "Status: decoding ({}x{}) ...",
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

            // stride(0) is bytes/row for RGBA-packed output in typical cases.
            let stride_u32 = out_rgba.stride(0);
            let stride_src = stride_u32 as usize;
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

            // keep latest for draw
            let _ = latest.lock().map(|mut g| {
                *g = Some((rgba_bytes.clone(), width, height));
            });

            let _ = tx.send((rgba_bytes, width, height));
        }
    }

    Ok(())
}
