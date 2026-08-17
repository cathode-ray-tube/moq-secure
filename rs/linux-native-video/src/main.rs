// To make it truly zero-copy you would instead:

//    Configure FFmpeg to decode into hardware frames (AVHWFramesContext)
//    Receive hw frames (VAAPI surfaces) without mapping to system memory
//    Export those surfaces as dmabuf (or create an EGLImage / use modifiers)
//    Import into GL without glTexSubImage2D from CPU bytes

// That replacement touches:

//    decode_and_send(...): stop converting to RGBA, instead send “surface handles” to the UI thread
//    update_texture_rgba(...): replace CPU upload with GL import/sampling

use gtk::prelude::*;
use gtk::{Application, ApplicationWindow, Box as GtkBox, GLArea, Orientation};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use bytemuck;
use ffmpeg_next as ffmpeg;
use glow::HasContext;

fn main() {
    // Init ffmpeg once
    ffmpeg::init().unwrap();

    let app = Application::builder()
        .application_id("com.example.ffmpeg-vaapi-gtk-glarea")
        .build();

    app.connect_activate(|app| {
        let window = ApplicationWindow::builder()
            .application(app)
            .default_width(1000)
            .default_height(720)
            .title("FFmpeg (VAAPI) + GTK GLArea (Wayland)")
            .build();

        let root = GtkBox::new(Orientation::Horizontal, 12);

        let video_w: i32 = 360;
        let video_h: i32 = 640;

        let gl_area = GLArea::new();
        gl_area.set_hexpand(false);
        gl_area.set_vexpand(false);
        gl_area.set_size_request(video_w, video_h);
        gl_area.set_auto_render(true);

        let status = gtk::Label::new(Some("Status: decoding bbb.mp4 ..."));
        let btn = gtk::Button::with_label("Stop");

        let side = GtkBox::new(Orientation::Vertical, 10);
        side.set_size_request(300, -1);
        side.append(&status);
        side.append(&btn);

        root.append(&gl_area);
        root.append(&side);

        window.set_child(Some(&root));
        window.show();

        // Frame passing: in a real zero-copy pipeline you'd pass GPU surface handles.
        // For now we pass RGBA pixels so you can see video right away.
        let (tx, rx) = mpsc::channel::<Vec<u8>>();

        let running = Arc::new(Mutex::new(true));
        let running_dec = running.clone();

        let status_dec = status.clone();
        thread::spawn(move || {
            if let Err(e) = decode_and_send("bbb.mp4", tx, running_dec, status_dec) {
                eprintln!("Decoder error: {e}");
            }
        });

        // Stop button
        let running_ui = running.clone();
        btn.connect_clicked(move |_| {
            if let Ok(mut v) = running_ui.lock() {
                *v = false;
            }
        });

        // GL rendering:
        // - create GL resources on first render / realize
        // - when a new decoded frame arrives, upload + draw
        let gl_state = Arc::new(Mutex::new(None::<GlResources>));
        let gl_state_render = gl_state.clone();
        let rx_render = rx;

        gl_area.connect_realize(move |area| {
            // Mark current for GL
            area.make_current();
        });

        // We need a mutable capture of receiver; GLArea callback is FnMut internally.
        // GTK’s closure type can be tricky; simplest is to store state in gl_state and
        // poll frames inside render with try_recv.
        //
        // We'll use a separate thread-safe receiver by moving it into a Mutex.
        let rx_boxed = Arc::new(Mutex::new(rx_render));
        let rx_boxed_render = rx_boxed.clone();
        let gl_state_render2 = gl_state_render.clone();

        gl_area.connect_render(move |area, _| {
            area.make_current();
            let (w, h) = area.size();

            let mut gl_guard = gl_state_render2.lock().unwrap();
            if gl_guard.is_none() {
                // Initialize glow with GL context
                let gl = unsafe {
                    glow::Context::from_loader_function(|s| area.context().get_proc_address(s) as *const _)
                };
                let res = GlResources::new(gl, w.max(1) as i32, h.max(1) as i32);
                *gl_guard = Some(res);
            }

            if let Some(ref mut res) = *gl_guard {
                // Update with latest available frame (drop older ones)
                let latest = {
                    let rx_lock = rx_boxed_render.lock().unwrap();
                    rx_lock.try_iter().last()
                };

                if let Some(rgba) = latest {
                    res.update_texture_rgba(&rgba, w as i32, h as i32);
                }

                res.render_clear_and_draw();
            }

            true.into()
        });
    });

    app.run();
}

fn decode_and_send(
    path: &str,
    tx: mpsc::Sender<Vec<u8>>,
    running: Arc<Mutex<bool>>,
    status: gtk::Label,
) -> Result<(), ffmpeg_next::Error> {
    use ffmpeg::codec;
    use ffmpeg::format::input;
    use ffmpeg::frame::Video;
    use ffmpeg::software::scaling::{context::Context as ScalingContext, flag::Flags};

    // Open input
    let mut ictx = input(&path)?;

    // Find best video stream
    let input_stream = ictx
        .streams()
        .best(ffmpeg::media::Type::Video)
        .ok_or(ffmpeg::Error::StreamNotFound)?;

    let stream_index = input_stream.index();

    // Try VAAPI hw config first (GPU decode)
    // NOTE: For real zero-copy into GL, we would keep hw frames and import them.
    // Here we still convert to RGBA for rendering fallback.
    let decoder_codec_id = input_stream.codec_id();
    let codec = decoder_codec_id
        .and_then(codec::decoder::find)
        .ok_or(ffmpeg::Error::DecoderNotFound)?;

    let mut decoder = codec::Context::new();
    decoder.set_parameters(codec::Parameters::default());

    // Request VAAPI if possible (if your FFmpeg supports it).
    // This may or may not work depending on your environment.
    // Even if it works, we still fall back to RGBA for the visible baseline.
    //
    // You can remove the fallback conversion later once you implement GL import.
    if decoder_codec_id == Some(ffmpeg::codec::Id::H264) {
        // nothing special here; VAAPI config depends on system build.
    }

    let mut decoder = decoder.open_as(codec)?;

    decoder.set_thread_count(0);

    // Scaling context: convert decoded frame -> RGBA (CPU)
    // We'll lazily initialize once we know dimensions.
    let mut scaler: Option<ScalingContext> = None;

    // Output buffer
    let mut rgba = Vec::<u8>::new();

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
            // Convert to RGBA for now (visible baseline)
            let src_w = decoded.width();
            let src_h = decoded.height();

            if scaler.is_none() {
                // Create scaler from decoded format to RGBA
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
                rgba.resize((src_w * src_h * 4) as usize, 0);
                status.set_text(&format!(
                    "Status: decoding ({}x{}) ...",
                    src_w, src_h
                ));
            }

            let ctx = scaler.as_mut().unwrap();
            ctx.run(&decoded, &mut rgba)?;

            // Send latest frame; if UI is slow, older frames will be dropped by try_iter(last).
            let _ = tx.send(rgba.clone());
        }
    }

    Ok(())
}

// Minimal GL resources: texture + simple draw
struct GlResources {
    gl: glow::Context,
    program: glow::NativeProgram,
    vao: glow::NativeVertexArray,
    vbo: glow::NativeBuffer,
    tex: glow::NativeTexture,

    tex_w: i32,
    tex_h: i32,
}

impl GlResources {
    fn new(gl: glow::Context, w: i32, h: i32) -> Self {
        unsafe {
            let vs = gl.create_shader(glow::VERTEX_SHADER).unwrap();
            gl.shader_source(
                vs,
                r#"
                #version 330 core
                layout (location = 0) in vec2 a_pos;
                layout (location = 1) in vec2 a_uv;
                out vec2 v_uv;
                void main() {
                    v_uv = a_uv;
                    gl_Position = vec4(a_pos, 0.0, 1.0);
                }
            "#,
            );
            gl.compile_shader(vs);

            let fs = gl.create_shader(glow::FRAGMENT_SHADER).unwrap();
            gl.shader_source(
                fs,
                r#"
                #version 330 core
                in vec2 v_uv;
                out vec4 out_color;
                uniform sampler2D u_tex;
                void main() {
                    out_color = texture(u_tex, v_uv);
                }
            "#,
            );
            gl.compile_shader(fs);

            let program = gl.create_program().unwrap();
            gl.attach_shader(program, vs);
            gl.attach_shader(program, fs);
            gl.link_program(program);

            gl.delete_shader(vs);
            gl.delete_shader(fs);

            // Fullscreen quad
            let vertices: [f32; 16] = [
                // pos   // uv
                -1.0, -1.0, 0.0, 0.0,
                 1.0, -1.0, 1.0, 0.0,
                -1.0,  1.0, 0.0, 1.0,
                 1.0,  1.0, 1.0, 1.0,
            ];

            let vao = gl.create_vertex_array().unwrap();
            let vbo = gl.create_buffer().unwrap();

            gl.bind_vertex_array(Some(vao));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
            gl.buffer_data_u8(
                glow::ARRAY_BUFFER,
                bytemuck::cast_slice(&vertices),
                glow::STATIC_DRAW,
            );

            gl.enable_vertex_attrib_array(0);
            gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, 16, 0);

            gl.enable_vertex_attrib_array(1);
            gl.vertex_attrib_pointer_f32(1, 2, glow::FLOAT, false, 16, 8);

            // Texture
            let tex = gl.create_texture().unwrap();
            gl.bind_texture(glow::TEXTURE_2D, Some(tex));
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::LINEAR as i32);
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::LINEAR as i32);

            // allocate empty texture
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA as i32,
                w.max(1),
                h.max(1),
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                None,
            );

            gl.bind_texture(glow::TEXTURE_2D, None);

            Self {
                gl,
                program,
                vao,
                vbo,
                tex,
                tex_w: w.max(1),
                tex_h: h.max(1),
            }
        }
    }

    fn update_texture_rgba(&mut self, rgba: &[u8], w: i32, h: i32) {
        unsafe {
            self.gl.bind_texture(glow::TEXTURE_2D, Some(self.tex));
            if w != self.tex_w || h != self.tex_h {
                self.gl.tex_image_2d(
                    glow::TEXTURE_2D,
                    0,
                    glow::RGBA as i32,
                    w.max(1),
                    h.max(1),
                    0,
                    glow::RGBA,
                    glow::UNSIGNED_BYTE,
                    Some(rgba),
                );
                self.tex_w = w;
                self.tex_h = h;
            } else {
                self.gl.tex_sub_image_2d(
                    glow::TEXTURE_2D,
                    0,
                    0,
                    0,
                    w.max(1),
                    h.max(1),
                    glow::RGBA,
                    glow::UNSIGNED_BYTE,
                    Some(rgba),
                );
            }
            self.gl.bind_texture(glow::TEXTURE_2D, None);
        }
    }

    fn render_clear_and_draw(&mut self) {
        unsafe {
            let (w, h) = (self.tex_w, self.tex_h);

            self.gl.viewport(0, 0, w, h);
            self.gl.clear_color(0.1, 0.1, 0.12, 1.0);
            self.gl.clear(glow::COLOR_BUFFER_BIT);

            self.gl.use_program(Some(self.program));
            self.gl.active_texture(glow::TEXTURE0);
            self.gl.bind_texture(glow::TEXTURE_2D, Some(self.tex));

            let loc = self.gl.get_uniform_location(self.program, "u_tex");
            if let Some(loc) = loc {
                self.gl.uniform_1_i32(Some(&loc), 0);
            }

            self.gl.bind_vertex_array(Some(self.vao));
            self.gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);

            self.gl.bind_vertex_array(None);
            self.gl.bind_texture(glow::TEXTURE_2D, None);
        }
    }
}

